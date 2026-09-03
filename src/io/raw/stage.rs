use std::{
    fs::File,
    io,
    os::fd::{AsRawFd, OwnedFd},
    path::Path,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

use rustix::fs::{self, AtFlags, FileType, Mode, OFlags};

use crate::io::{DecodeError, DigestError, ResourceLimits, SourceDigestV1, SourceFileIdentity};

pub(super) struct StagedRaw {
    base: OwnedFd,
    directory: OwnedFd,
    directory_name: String,
    input_name: String,
    output_name: String,
    pub digest: SourceDigestV1,
    pub identity: SourceFileIdentity,
}

impl StagedRaw {
    pub fn input_path(&self) -> String {
        format!(
            "/proc/self/fd/{}/{}",
            self.directory.as_raw_fd(),
            self.input_name
        )
    }
    pub fn output_path(&self) -> String {
        format!(
            "/proc/self/fd/{}/{}",
            self.directory.as_raw_fd(),
            self.output_name
        )
    }
    pub fn open_input(&self) -> Result<File, DecodeError> {
        self.open_member(self.input_name.as_str())
    }
    pub fn open_output(&self) -> Result<File, DecodeError> {
        self.open_member(self.output_name.as_str())
    }
    fn open_member(&self, name: &str) -> Result<File, DecodeError> {
        let fd = fs::openat(
            &self.directory,
            name,
            OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::empty(),
        )
        .map_err(|e| DecodeError::RawBackendCaptureIo(std_error(e)))?;
        let stat = fs::fstat(&fd).map_err(|e| DecodeError::RawBackendCaptureIo(std_error(e)))?;
        if !FileType::from_raw_mode(stat.st_mode).is_file() {
            return Err(DecodeError::CorruptInput);
        }
        Ok(File::from(fd))
    }
}

impl Drop for StagedRaw {
    fn drop(&mut self) {
        let _ = fs::unlinkat(&self.directory, self.input_name.as_str(), AtFlags::empty());
        let _ = fs::unlinkat(&self.directory, self.output_name.as_str(), AtFlags::empty());
        let _ = fs::unlinkat(&self.base, self.directory_name.as_str(), AtFlags::REMOVEDIR);
    }
}

pub(super) fn stage_source(
    source: &Path,
    base_path: &Path,
    limits: &ResourceLimits,
    cancelled: &Arc<AtomicBool>,
) -> Result<StagedRaw, DecodeError> {
    if cancelled.load(Ordering::Acquire) {
        return Err(DecodeError::Cancelled);
    }
    let source_fd = fs::open(
        source,
        OFlags::RDONLY | OFlags::NONBLOCK | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::empty(),
    )
    .map_err(map_source_open)?;
    stage_source_file(File::from(source_fd), source, base_path, limits, cancelled)
}

pub(super) fn stage_source_file(
    source: File,
    source_name: &Path,
    base_path: &Path,
    limits: &ResourceLimits,
    cancelled: &Arc<AtomicBool>,
) -> Result<StagedRaw, DecodeError> {
    let suffix = safe_suffix(source_name);
    let source_stat = fs::fstat(&source).map_err(|e| DecodeError::Input(std_error(e)))?;
    if !FileType::from_raw_mode(source_stat.st_mode).is_file() {
        return Err(DecodeError::UnsupportedFormat);
    }
    let source_size =
        u64::try_from(source_stat.st_size).map_err(|_| DecodeError::UnsupportedFormat)?;
    limits
        .check_source_bytes(source_size)
        .map_err(DecodeError::Limit)?;
    let source_identity =
        SourceFileIdentity::from_device_inode(source_stat.st_dev, source_stat.st_ino);

    let base = fs::open(
        base_path,
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::empty(),
    )
    .map_err(|e| DecodeError::Input(std_error(e)))?;
    for _ in 0..16 {
        let directory_name = format!(".omalux-raw-{}", random_hex()?);
        match fs::mkdirat(&base, directory_name.as_str(), Mode::RWXU) {
            Ok(()) => {}
            Err(error) if error == rustix::io::Errno::EXIST => continue,
            Err(error) => return Err(DecodeError::Input(std_error(error))),
        }
        let mut setup = SetupGuard {
            base: &base,
            directory_name: &directory_name,
            armed: true,
        };
        let directory_path_fd = fs::openat(
            &base,
            directory_name.as_str(),
            OFlags::PATH | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::empty(),
        )
        .map_err(|e| DecodeError::Input(std_error(e)))?;
        std::fs::set_permissions(
            format!("/proc/self/fd/{}", directory_path_fd.as_raw_fd()),
            <std::fs::Permissions as std::os::unix::fs::PermissionsExt>::from_mode(0o700),
        )
        .map_err(DecodeError::Input)?;
        drop(directory_path_fd);
        let directory = fs::openat(
            &base,
            directory_name.as_str(),
            OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::empty(),
        )
        .map_err(|e| DecodeError::Input(std_error(e)))?;
        set_and_verify_mode(&directory, Mode::RWXU)?;
        rustix::io::fcntl_setfd(&directory, rustix::io::FdFlags::empty())
            .map_err(|e| DecodeError::Input(std_error(e)))?;
        setup.armed = false;
        drop(setup);
        return finish_stage(
            base,
            directory,
            directory_name,
            (source.into(), source_identity),
            suffix,
            limits,
            cancelled,
        );
    }
    Err(DecodeError::Input(io::Error::new(
        io::ErrorKind::AlreadyExists,
        "secure stage directories exhausted",
    )))
}

// Kept separate so the cleanup guard is installed immediately after mkdirat.
fn finish_stage(
    base: OwnedFd,
    directory: OwnedFd,
    directory_name: String,
    source: (OwnedFd, SourceFileIdentity),
    suffix: String,
    limits: &ResourceLimits,
    cancelled: &Arc<AtomicBool>,
) -> Result<StagedRaw, DecodeError> {
    let (source, source_identity) = source;
    let input_name = format!("input.{suffix}");
    let output_name = "output.ppm".to_owned();
    let mut guard = OwnedSetupGuard {
        base: &base,
        directory: &directory,
        directory_name: &directory_name,
        input: Some(&input_name),
        output: None,
        armed: true,
    };
    let input = fs::openat(
        &directory,
        input_name.as_str(),
        OFlags::WRONLY | OFlags::CREATE | OFlags::EXCL | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::RUSR | Mode::WUSR,
    )
    .map_err(|e| DecodeError::Input(std_error(e)))?;
    set_and_verify_mode(&input, Mode::RUSR | Mode::WUSR)?;
    let output = fs::openat(
        &directory,
        output_name.as_str(),
        OFlags::WRONLY | OFlags::CREATE | OFlags::EXCL | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::RUSR | Mode::WUSR,
    )
    .map_err(|e| DecodeError::Input(std_error(e)))?;
    guard.output = Some(&output_name);
    set_and_verify_mode(&output, Mode::RUSR | Mode::WUSR)?;
    drop(output);
    let mut source = File::from(source);
    let mut input = File::from(input);
    let result = SourceDigestV1::copy_from_reader(&mut source, &mut input, limits, || {
        cancelled.load(Ordering::Acquire)
    });
    let digest = match result {
        Ok((digest, _)) => digest,
        Err(error) => return Err(map_digest(error)),
    };
    input.sync_all().map_err(DecodeError::Input)?;
    fs::fsync(&directory).map_err(|e| DecodeError::Input(std_error(e)))?;
    drop(input);
    guard.armed = false;
    drop(guard);
    Ok(StagedRaw {
        base,
        directory,
        directory_name,
        input_name,
        output_name,
        digest,
        identity: source_identity,
    })
}

// Temporary guard used between mkdir and ownership transfer.
struct SetupGuard<'a> {
    base: &'a OwnedFd,
    directory_name: &'a str,
    armed: bool,
}
impl Drop for SetupGuard<'_> {
    fn drop(&mut self) {
        if self.armed {
            let _ = fs::unlinkat(self.base, self.directory_name, AtFlags::REMOVEDIR);
        }
    }
}
struct OwnedSetupGuard<'a> {
    base: &'a OwnedFd,
    directory: &'a OwnedFd,
    directory_name: &'a str,
    input: Option<&'a str>,
    output: Option<&'a str>,
    armed: bool,
}
impl Drop for OwnedSetupGuard<'_> {
    fn drop(&mut self) {
        if self.armed {
            if let Some(name) = self.input {
                let _ = fs::unlinkat(self.directory, name, AtFlags::empty());
            }
            if let Some(name) = self.output {
                let _ = fs::unlinkat(self.directory, name, AtFlags::empty());
            }
            let _ = fs::unlinkat(self.base, self.directory_name, AtFlags::REMOVEDIR);
        }
    }
}

fn safe_suffix(path: &Path) -> String {
    path.extension()
        .and_then(|s| s.to_str())
        .filter(|s| !s.is_empty() && s.len() <= 12 && s.bytes().all(|b| b.is_ascii_alphanumeric()))
        .map(|s| s.to_ascii_lowercase())
        .unwrap_or_else(|| "raw".into())
}
fn random_hex() -> Result<String, DecodeError> {
    let mut bytes = [0_u8; 16];
    getrandom::fill(&mut bytes).map_err(|e| DecodeError::Input(io::Error::other(e)))?;
    Ok(bytes.iter().map(|b| format!("{b:02x}")).collect())
}
fn std_error(error: rustix::io::Errno) -> io::Error {
    io::Error::from_raw_os_error(error.raw_os_error())
}
fn set_and_verify_mode(fd: &OwnedFd, expected: Mode) -> Result<(), DecodeError> {
    fs::fchmod(fd, expected).map_err(|e| DecodeError::Input(std_error(e)))?;
    let actual = fs::fstat(fd).map_err(|e| DecodeError::Input(std_error(e)))?;
    if actual.st_mode & 0o7777 != expected.bits() {
        return Err(DecodeError::Input(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "private staging mode could not be established",
        )));
    }
    Ok(())
}
fn map_source_open(error: rustix::io::Errno) -> DecodeError {
    if error == rustix::io::Errno::LOOP {
        DecodeError::UnsupportedFormat
    } else {
        DecodeError::Input(std_error(error))
    }
}
fn map_digest(error: DigestError) -> DecodeError {
    match error {
        DigestError::Read(e) if e.kind() == io::ErrorKind::Interrupted => DecodeError::Cancelled,
        DigestError::Read(e) => DecodeError::Input(e),
        DigestError::Limit(e) => DecodeError::Limit(e),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        fs as stdfs,
        io::Read,
        os::unix::{fs::PermissionsExt, net::UnixListener},
        time::{Duration, Instant},
    };
    use tempfile::tempdir;

    fn cancellation() -> Arc<AtomicBool> {
        Arc::new(AtomicBool::new(false))
    }

    #[test]
    fn fifo_socket_and_symlink_are_rejected_without_blocking() {
        let directory = tempdir().unwrap();
        let fifo = directory.path().join("fifo.raw");
        rustix::fs::mkfifoat(rustix::fs::CWD, &fifo, Mode::RUSR | Mode::WUSR).unwrap();
        let socket = directory.path().join("socket.raw");
        let _listener = UnixListener::bind(&socket).unwrap();
        let regular = directory.path().join("regular.raw");
        stdfs::write(&regular, b"raw").unwrap();
        let symlink = directory.path().join("symlink.raw");
        std::os::unix::fs::symlink(&regular, &symlink).unwrap();
        for source in [&fifo, &socket, &symlink] {
            let start = Instant::now();
            assert!(
                stage_source(
                    source,
                    directory.path(),
                    &ResourceLimits::default(),
                    &cancellation()
                )
                .is_err()
            );
            assert!(start.elapsed() < Duration::from_secs(1));
        }
    }

    #[test]
    fn held_directory_survives_parent_rename_and_drop_cleans_it() {
        let root = tempdir().unwrap();
        let source = root.path().join("source.NEF");
        stdfs::write(&source, b"immutable source bytes").unwrap();
        let stage = root.path().join("stage");
        let moved = root.path().join("moved");
        stdfs::create_dir(&stage).unwrap();
        let staged =
            stage_source(&source, &stage, &ResourceLimits::default(), &cancellation()).unwrap();
        stdfs::rename(&stage, &moved).unwrap();
        let mut bytes = Vec::new();
        File::open(staged.input_path())
            .unwrap()
            .read_to_end(&mut bytes)
            .unwrap();
        assert_eq!(bytes, b"immutable source bytes");
        let session = moved.join(&staged.directory_name);
        assert_eq!(
            stdfs::metadata(&session).unwrap().permissions().mode() & 0o7777,
            0o700
        );
        for name in [&staged.input_name, &staged.output_name] {
            assert_eq!(
                stdfs::metadata(session.join(name))
                    .unwrap()
                    .permissions()
                    .mode()
                    & 0o7777,
                0o600
            );
        }
        drop(staged);
        assert_eq!(stdfs::read_dir(moved).unwrap().count(), 0);
    }

    #[test]
    fn source_limit_failure_leaves_no_stage_entries() {
        let root = tempdir().unwrap();
        let source = root.path().join("source.raw");
        stdfs::write(&source, [0_u8; 16]).unwrap();
        let limits = ResourceLimits {
            max_source_bytes: 8,
            ..ResourceLimits::default()
        };
        assert!(matches!(
            stage_source(&source, root.path(), &limits, &cancellation()),
            Err(DecodeError::Limit(_))
        ));
        assert_eq!(
            stdfs::read_dir(root.path())
                .unwrap()
                .filter_map(Result::ok)
                .filter(|entry| entry
                    .file_name()
                    .to_string_lossy()
                    .starts_with(".omalux-raw-"))
                .count(),
            0
        );
    }

    #[test]
    fn restrictive_umask_child() {
        if std::env::var_os("OMALUX_RAW_UMASK_CHILD").is_none() {
            return;
        }
        let root = tempdir().unwrap();
        let source = root.path().join("source.raw");
        stdfs::write(&source, b"raw").unwrap();
        let old = rustix::process::umask(Mode::from_bits_retain(0o777));
        let staged = stage_source(
            &source,
            root.path(),
            &ResourceLimits::default(),
            &cancellation(),
        )
        .unwrap();
        let session = root.path().join(&staged.directory_name);
        assert_eq!(
            stdfs::metadata(&session).unwrap().permissions().mode() & 0o7777,
            0o700
        );
        for name in [&staged.input_name, &staged.output_name] {
            assert_eq!(
                stdfs::metadata(session.join(name))
                    .unwrap()
                    .permissions()
                    .mode()
                    & 0o7777,
                0o600
            );
        }
        rustix::process::umask(old);
    }

    #[test]
    fn restrictive_umask_still_produces_exact_private_modes() {
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact", "io::raw::stage::tests::restrictive_umask_child"])
            .env("OMALUX_RAW_UMASK_CHILD", "1")
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
}
