use std::{fs::File, io, io::Write, path::Path};

use super::AtomicOutputError;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum OverwritePolicy {
    /// Refuse to publish when the destination already exists.
    ///
    /// This is the required default for future production encoders.
    Forbid,
    /// Atomically replace an existing regular destination file.
    ///
    /// This policy is only suitable when the destination directory is trusted
    /// and all writers to the destination name cooperate. Holding the parent
    /// directory descriptor prevents path redirection, but it cannot prevent a
    /// cooperating-directory violation such as another writer publishing a
    /// newer file immediately before this rename. `Replace` is therefore an
    /// explicit opt-in and is not the production default.
    Replace,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum OutputPermissions {
    Private,
    PreserveExisting,
    Mode(u32),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub struct AtomicOutputOptions {
    pub overwrite: OverwritePolicy,
    pub permissions: OutputPermissions,
}

impl Default for AtomicOutputOptions {
    fn default() -> Self {
        Self {
            // Future encoder entry points must retain this fail-closed default
            // unless the caller explicitly opts into the trusted-directory
            // `Replace` contract above.
            overwrite: OverwritePolicy::Forbid,
            permissions: OutputPermissions::Private,
        }
    }
}

impl AtomicOutputOptions {
    pub fn with_overwrite(mut self, overwrite: OverwritePolicy) -> Self {
        self.overwrite = overwrite;
        self
    }
}

/// Publication status after the destination commit point.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum AtomicOutputOutcome {
    PublishedAndDurable,
    /// The destination is visible but directory durability could not be
    /// confirmed. Retrying blindly can overwrite a successfully published
    /// file.
    PublishedButNotDurable,
}

/// Stable identity captured from the decoder's already-open source file.
///
/// Publication compares this value with the destination's `lstat` result; it
/// never resolves the input path again. The decoder should retain its source
/// handle for the job lifetime when its backend permits that.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SourceFileIdentity {
    device: u64,
    inode: u64,
}

impl SourceFileIdentity {
    pub(crate) const fn from_device_inode(device: u64, inode: u64) -> Self {
        Self { device, inode }
    }

    #[cfg(target_os = "linux")]
    pub fn from_file(file: &File) -> Result<Self, AtomicOutputError> {
        let stat =
            rustix::fs::fstat(file).map_err(|error| AtomicOutputError::Create(std_error(error)))?;
        Ok(Self {
            device: stat.st_dev,
            inode: stat.st_ino,
        })
    }

    #[cfg(not(target_os = "linux"))]
    pub fn from_file(_file: &File) -> Result<Self, AtomicOutputError> {
        Err(AtomicOutputError::UnsupportedPlatform)
    }
}

/// Atomically writes a new regular file on Linux without following the final
/// parent or destination symlink. All name operations use one held directory
/// descriptor, so replacing the path to that directory cannot redirect work.
///
/// A post-rename directory-sync failure returns
/// [`AtomicOutputError::PublishedButNotDurable`]; that error is never safe to
/// retry blindly because the destination is known to have been published.
pub fn write_atomic_output(
    destination: impl AsRef<Path>,
    input: Option<&Path>,
    options: AtomicOutputOptions,
    writer: impl FnOnce(&mut File) -> io::Result<()>,
) -> Result<AtomicOutputOutcome, AtomicOutputError> {
    #[cfg(target_os = "linux")]
    {
        let identity = input.map(source_identity_from_path).transpose()?;
        write_atomic_linux(
            destination.as_ref(),
            identity,
            options,
            writer,
            &mut NoFaults,
        )
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = (destination, input, options, writer);
        Err(AtomicOutputError::UnsupportedPlatform)
    }
}

/// Atomically publishes output while comparing against an identity captured
/// by the decoder, without reopening or resolving the source path.
pub fn write_atomic_output_for_source(
    destination: impl AsRef<Path>,
    source: Option<SourceFileIdentity>,
    options: AtomicOutputOptions,
    writer: impl FnOnce(&mut File) -> io::Result<()>,
) -> Result<AtomicOutputOutcome, AtomicOutputError> {
    #[cfg(target_os = "linux")]
    {
        write_atomic_linux(destination.as_ref(), source, options, writer, &mut NoFaults)
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = (destination, source, options, writer);
        Err(AtomicOutputError::UnsupportedPlatform)
    }
}

#[cfg(target_os = "linux")]
fn write_atomic_linux(
    destination: &Path,
    source: Option<SourceFileIdentity>,
    options: AtomicOutputOptions,
    writer: impl FnOnce(&mut File) -> io::Result<()>,
    hooks: &mut impl FaultHooks,
) -> Result<AtomicOutputOutcome, AtomicOutputError> {
    use rustix::fs::{self, AtFlags, FileType, Mode, OFlags, RenameFlags};

    let (parent, basename) = split_destination(destination)?;
    let directory = fs::open(
        parent,
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::empty(),
    )
    .map_err(|e| AtomicOutputError::Create(std_error(e)))?;
    hooks
        .after_parent_open()
        .map_err(AtomicOutputError::Create)?;

    let existing = match fs::statat(&directory, basename, AtFlags::SYMLINK_NOFOLLOW) {
        Ok(stat) => Some(stat),
        Err(e) if e == rustix::io::Errno::NOENT => None,
        Err(e) => return Err(AtomicOutputError::Create(std_error(e))),
    };
    if options.overwrite == OverwritePolicy::Forbid && existing.is_some() {
        return Err(AtomicOutputError::DestinationExists);
    }
    if options.overwrite == OverwritePolicy::Replace
        && existing
            .as_ref()
            .is_some_and(|s| !FileType::from_raw_mode(s.st_mode).is_file())
    {
        return Err(AtomicOutputError::InvalidDestinationType);
    }
    if let (Some(source), Some(output)) = (source, existing.as_ref()) {
        reject_collision(source, output)?;
    }

    let mode = permission_mode(options.permissions, existing.as_ref().map(|s| s.st_mode))?;
    let (temporary_name, owned) = create_temporary_at(&directory)?;
    let mut guard = TemporaryGuard {
        directory: &directory,
        name: &temporary_name,
        published: false,
    };
    hooks
        .after_temp_created()
        .map_err(AtomicOutputError::Write)?;
    let mut temporary = File::from(owned);
    writer(&mut temporary).map_err(AtomicOutputError::Write)?;
    temporary.flush().map_err(AtomicOutputError::Write)?;
    fs::fchmod(&temporary, mode).map_err(|e| AtomicOutputError::Write(std_error(e)))?;
    fs::fsync(&temporary).map_err(|e| AtomicOutputError::Sync(std_error(e)))?;
    drop(temporary);
    hooks.before_publish().map_err(AtomicOutputError::Publish)?;

    let publish = match options.overwrite {
        OverwritePolicy::Forbid => fs::renameat_with(
            &directory,
            temporary_name.as_str(),
            &directory,
            basename,
            RenameFlags::NOREPLACE,
        ),
        OverwritePolicy::Replace => {
            fs::renameat(&directory, temporary_name.as_str(), &directory, basename)
        }
    };
    if let Err(error) = publish {
        return Err(if error == rustix::io::Errno::EXIST {
            AtomicOutputError::DestinationExists
        } else {
            AtomicOutputError::Publish(std_error(error))
        });
    }
    guard.published = true;
    hooks
        .after_publish()
        .map_err(AtomicOutputError::PublishedButNotDurable)?;
    fs::fsync(&directory).map_err(|e| AtomicOutputError::PublishedButNotDurable(std_error(e)))?;
    Ok(AtomicOutputOutcome::PublishedAndDurable)
}

#[cfg(target_os = "linux")]
fn split_destination(destination: &Path) -> Result<(&Path, &std::ffi::OsStr), AtomicOutputError> {
    use std::path::Component;
    let basename = match destination.components().next_back() {
        Some(Component::Normal(name)) => name,
        _ => return Err(AtomicOutputError::InvalidDestination),
    };
    let parent = destination
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    Ok((parent, basename))
}

#[cfg(target_os = "linux")]
fn permission_mode(
    policy: OutputPermissions,
    existing: Option<u32>,
) -> Result<rustix::fs::Mode, AtomicOutputError> {
    let raw = match policy {
        OutputPermissions::Private => 0o600,
        OutputPermissions::PreserveExisting => existing.map_or(0o600, |m| m & 0o777),
        OutputPermissions::Mode(mode) if mode <= 0o777 => mode,
        OutputPermissions::Mode(_) => return Err(AtomicOutputError::InvalidPermissions),
    };
    Ok(rustix::fs::Mode::from_raw_mode(raw))
}

#[cfg(target_os = "linux")]
fn random_temporary_name() -> Result<String, AtomicOutputError> {
    let mut bytes = [0_u8; 16];
    getrandom::fill(&mut bytes).map_err(|e| AtomicOutputError::Create(io::Error::other(e)))?;
    let mut name = String::from(".omalux-output-");
    for byte in bytes {
        use std::fmt::Write as _;
        write!(&mut name, "{byte:02x}").expect("string writes cannot fail");
    }
    name.push_str(".tmp");
    Ok(name)
}

#[cfg(target_os = "linux")]
fn create_temporary_at(
    directory: &impl std::os::fd::AsFd,
) -> Result<(String, std::os::fd::OwnedFd), AtomicOutputError> {
    use rustix::fs::{self, Mode, OFlags};
    for _ in 0..16 {
        let name = random_temporary_name()?;
        match fs::openat(
            directory,
            name.as_str(),
            OFlags::WRONLY | OFlags::CREATE | OFlags::EXCL | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::RUSR | Mode::WUSR,
        ) {
            Ok(file) => return Ok((name, file)),
            Err(error) if error == rustix::io::Errno::EXIST => continue,
            Err(error) => return Err(AtomicOutputError::Create(std_error(error))),
        }
    }
    Err(AtomicOutputError::Create(io::Error::new(
        io::ErrorKind::AlreadyExists,
        "secure temporary names exhausted",
    )))
}

#[cfg(target_os = "linux")]
fn source_identity_from_path(path: &Path) -> Result<SourceFileIdentity, AtomicOutputError> {
    use rustix::fs::{self, Mode, OFlags};
    let input = fs::open(path, OFlags::PATH | OFlags::CLOEXEC, Mode::empty())
        .map_err(|e| AtomicOutputError::Create(std_error(e)))?;
    SourceFileIdentity::from_file(&File::from(input))
}

#[cfg(target_os = "linux")]
fn reject_collision(
    source: SourceFileIdentity,
    output: &rustix::fs::Stat,
) -> Result<(), AtomicOutputError> {
    if source.device == output.st_dev && source.inode == output.st_ino {
        return Err(AtomicOutputError::InputOutputCollision);
    }
    Ok(())
}

#[cfg(target_os = "linux")]
fn std_error(error: rustix::io::Errno) -> io::Error {
    io::Error::from_raw_os_error(error.raw_os_error())
}

#[cfg(target_os = "linux")]
trait FaultHooks {
    fn after_parent_open(&mut self) -> io::Result<()> {
        Ok(())
    }
    fn after_temp_created(&mut self) -> io::Result<()> {
        Ok(())
    }
    fn before_publish(&mut self) -> io::Result<()> {
        Ok(())
    }
    fn after_publish(&mut self) -> io::Result<()> {
        Ok(())
    }
}
#[cfg(target_os = "linux")]
struct NoFaults;
#[cfg(target_os = "linux")]
impl FaultHooks for NoFaults {}

#[cfg(target_os = "linux")]
struct TemporaryGuard<'a, F: std::os::fd::AsFd> {
    directory: &'a F,
    name: &'a str,
    published: bool,
}
#[cfg(target_os = "linux")]
impl<F: std::os::fd::AsFd> Drop for TemporaryGuard<'_, F> {
    fn drop(&mut self) {
        if !self.published {
            let _ = rustix::fs::unlinkat(self.directory, self.name, rustix::fs::AtFlags::empty());
        }
    }
}

#[cfg(all(test, target_os = "linux"))]
mod tests {
    use super::*;
    use std::{
        fs,
        os::unix::{fs::PermissionsExt, net::UnixListener},
        sync::{Arc, Barrier},
        thread,
    };
    use tempfile::tempdir;

    fn opts(overwrite: OverwritePolicy) -> AtomicOutputOptions {
        AtomicOutputOptions {
            overwrite,
            permissions: OutputPermissions::Private,
        }
    }
    #[test]
    fn bare_name_uses_dot_parent() {
        let (p, n) = split_destination(Path::new("out")).unwrap();
        assert_eq!(p, Path::new("."));
        assert_eq!(n, "out");
    }
    #[test]
    fn success_is_private_and_durable() {
        let d = tempdir().unwrap();
        let out = d.path().join("out");
        assert_eq!(
            write_atomic_output(&out, None, Default::default(), |f| f.write_all(b"ok")).unwrap(),
            AtomicOutputOutcome::PublishedAndDurable
        );
        assert_eq!(fs::read(&out).unwrap(), b"ok");
        assert_eq!(
            fs::metadata(out).unwrap().permissions().mode() & 0o777,
            0o600
        );
    }
    #[test]
    fn writer_failure_cleans_temp() {
        let d = tempdir().unwrap();
        let out = d.path().join("out");
        assert!(matches!(
            write_atomic_output(&out, None, Default::default(), |f| {
                f.write_all(b"partial")?;
                Err(io::Error::other("fault"))
            }),
            Err(AtomicOutputError::Write(_))
        ));
        assert_eq!(fs::read_dir(d.path()).unwrap().count(), 0);
    }
    #[test]
    fn replace_rejects_symlink_and_socket() {
        use std::os::unix::fs::symlink;
        let d = tempdir().unwrap();
        let target = d.path().join("target");
        fs::write(&target, b"old").unwrap();
        let link = d.path().join("link");
        symlink(&target, &link).unwrap();
        assert!(matches!(
            write_atomic_output(&link, None, opts(OverwritePolicy::Replace), |f| f
                .write_all(b"new")),
            Err(AtomicOutputError::InvalidDestinationType)
        ));
        let socket = d.path().join("socket");
        let _listener = UnixListener::bind(&socket).unwrap();
        assert!(matches!(
            write_atomic_output(&socket, None, opts(OverwritePolicy::Replace), |f| f
                .write_all(b"new")),
            Err(AtomicOutputError::InvalidDestinationType)
        ));
    }
    #[test]
    fn hardlink_input_collision_is_rejected() {
        let d = tempdir().unwrap();
        let input = d.path().join("input");
        let out = d.path().join("out");
        fs::write(&input, b"x").unwrap();
        fs::hard_link(&input, &out).unwrap();
        assert!(matches!(
            write_atomic_output(&out, Some(&input), opts(OverwritePolicy::Replace), |f| f
                .write_all(b"new")),
            Err(AtomicOutputError::InputOutputCollision)
        ));
    }

    #[test]
    fn captured_source_identity_rejects_same_file_and_hardlink_without_reopening_source() {
        let directory = tempdir().unwrap();
        let input = directory.path().join("input");
        let hardlink = directory.path().join("hardlink");
        fs::write(&input, b"source").unwrap();
        fs::hard_link(&input, &hardlink).unwrap();
        let held = File::open(&input).unwrap();
        let identity = SourceFileIdentity::from_file(&held).unwrap();

        for destination in [&input, &hardlink] {
            assert!(matches!(
                write_atomic_output_for_source(
                    destination,
                    Some(identity),
                    opts(OverwritePolicy::Forbid),
                    |file| file.write_all(b"new")
                ),
                Err(AtomicOutputError::DestinationExists)
            ));
            assert!(matches!(
                write_atomic_output_for_source(
                    destination,
                    Some(identity),
                    opts(OverwritePolicy::Replace),
                    |file| file.write_all(b"new")
                ),
                Err(AtomicOutputError::InputOutputCollision)
            ));
        }
        assert_eq!(fs::read(input).unwrap(), b"source");
        assert_eq!(fs::read(hardlink).unwrap(), b"source");
    }

    #[test]
    fn captured_source_identity_keeps_symlink_policies_fail_closed() {
        use std::os::unix::fs::symlink;

        let directory = tempdir().unwrap();
        let input = directory.path().join("input");
        let link = directory.path().join("link");
        fs::write(&input, b"source").unwrap();
        symlink(&input, &link).unwrap();
        let held = File::open(&input).unwrap();
        let identity = SourceFileIdentity::from_file(&held).unwrap();

        assert!(matches!(
            write_atomic_output_for_source(
                &link,
                Some(identity),
                opts(OverwritePolicy::Forbid),
                |file| file.write_all(b"new")
            ),
            Err(AtomicOutputError::DestinationExists)
        ));
        assert!(matches!(
            write_atomic_output_for_source(
                &link,
                Some(identity),
                opts(OverwritePolicy::Replace),
                |file| file.write_all(b"new")
            ),
            Err(AtomicOutputError::InvalidDestinationType)
        ));
        assert_eq!(fs::read(input).unwrap(), b"source");
    }
    #[test]
    fn concurrent_forbid_has_exactly_one_winner() {
        let d = tempdir().unwrap();
        let out = Arc::new(d.path().join("out"));
        let barrier = Arc::new(Barrier::new(12));
        let handles = (0..12)
            .map(|i| {
                let out = out.clone();
                let barrier = barrier.clone();
                thread::spawn(move || {
                    barrier.wait();
                    write_atomic_output(&*out, None, Default::default(), |f| f.write_all(&[i]))
                })
            })
            .collect::<Vec<_>>();
        let results = handles
            .into_iter()
            .map(|h| h.join().unwrap())
            .collect::<Vec<_>>();
        assert_eq!(results.iter().filter(|r| r.is_ok()).count(), 1);
        assert!(
            results
                .iter()
                .filter(|r| r.is_err())
                .all(|r| matches!(r, Err(AtomicOutputError::DestinationExists)))
        );
    }
    struct BeforePublishFault;
    impl FaultHooks for BeforePublishFault {
        fn before_publish(&mut self) -> io::Result<()> {
            Err(io::Error::other("pre"))
        }
    }
    #[test]
    fn prepublish_fault_has_no_destination_or_temp() {
        let d = tempdir().unwrap();
        let out = d.path().join("out");
        let result = write_atomic_linux(
            &out,
            None,
            Default::default(),
            |f| f.write_all(b"x"),
            &mut BeforePublishFault,
        );
        assert!(matches!(result, Err(AtomicOutputError::Publish(_))));
        assert_eq!(fs::read_dir(d.path()).unwrap().count(), 0);
    }
    struct AfterPublishFault;
    impl FaultHooks for AfterPublishFault {
        fn after_publish(&mut self) -> io::Result<()> {
            Err(io::Error::other("post"))
        }
    }
    #[test]
    fn postpublish_fault_is_explicit_and_destination_exists() {
        let d = tempdir().unwrap();
        let out = d.path().join("out");
        let result = write_atomic_linux(
            &out,
            None,
            Default::default(),
            |f| f.write_all(b"x"),
            &mut AfterPublishFault,
        );
        assert!(matches!(
            result,
            Err(AtomicOutputError::PublishedButNotDurable(_))
        ));
        assert_eq!(fs::read(out).unwrap(), b"x");
    }
    struct ParentSwap {
        old: std::path::PathBuf,
        moved: std::path::PathBuf,
    }
    impl FaultHooks for ParentSwap {
        fn after_parent_open(&mut self) -> io::Result<()> {
            fs::rename(&self.old, &self.moved)?;
            fs::create_dir(&self.old)
        }
    }
    #[test]
    fn parent_swap_cannot_redirect_dirfd_operations() {
        let root = tempdir().unwrap();
        let old = root.path().join("parent");
        let moved = root.path().join("held");
        fs::create_dir(&old).unwrap();
        let out = old.join("out");
        let result = write_atomic_linux(
            &out,
            None,
            Default::default(),
            |f| f.write_all(b"safe"),
            &mut ParentSwap {
                old: old.clone(),
                moved: moved.clone(),
            },
        );
        assert!(result.is_ok());
        assert!(!old.join("out").exists());
        assert_eq!(fs::read(moved.join("out")).unwrap(), b"safe");
    }
    #[test]
    fn replace_preserves_permissions() {
        let d = tempdir().unwrap();
        let out = d.path().join("out");
        fs::write(&out, b"old").unwrap();
        fs::set_permissions(&out, fs::Permissions::from_mode(0o640)).unwrap();
        write_atomic_output(
            &out,
            None,
            AtomicOutputOptions {
                overwrite: OverwritePolicy::Replace,
                permissions: OutputPermissions::PreserveExisting,
            },
            |f| f.write_all(b"new"),
        )
        .unwrap();
        assert_eq!(
            fs::metadata(out).unwrap().permissions().mode() & 0o777,
            0o640
        );
    }
}
