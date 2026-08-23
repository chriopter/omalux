use std::{
    fs::{self, File, OpenOptions, Permissions},
    io::{self, Write},
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

#[cfg(unix)]
use std::os::unix::fs::{MetadataExt, OpenOptionsExt, PermissionsExt};

use super::AtomicOutputError;

static TEMP_SEQUENCE: AtomicU64 = AtomicU64::new(0);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OverwritePolicy {
    Forbid,
    Replace,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OutputPermissions {
    /// Owner read/write only, independent of process umask.
    Private,
    /// Preserve an existing destination's mode; new files remain private.
    PreserveExisting,
    /// Explicit Unix permission bits (0o000 through 0o777).
    Mode(u32),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AtomicOutputOptions {
    pub overwrite: OverwritePolicy,
    pub permissions: OutputPermissions,
}

impl Default for AtomicOutputOptions {
    fn default() -> Self {
        Self {
            overwrite: OverwritePolicy::Forbid,
            permissions: OutputPermissions::Private,
        }
    }
}

/// Writes to a private temporary file in the destination directory, syncs
/// data, then atomically publishes it. `Forbid` uses an atomic hard-link
/// publication so a concurrent creator can never be overwritten; `Replace`
/// uses same-filesystem rename. The parent directory is synced after publish.
pub fn write_atomic_output(
    destination: impl AsRef<Path>,
    input: Option<&Path>,
    options: AtomicOutputOptions,
    writer: impl FnOnce(&mut File) -> io::Result<()>,
) -> Result<(), AtomicOutputError> {
    let destination = destination.as_ref();
    let parent = destination
        .parent()
        .filter(|path| !path.as_os_str().is_empty())
        .ok_or(AtomicOutputError::InvalidDestination)?;
    if destination.file_name().is_none() {
        return Err(AtomicOutputError::InvalidDestination);
    }

    let destination_metadata = fs::symlink_metadata(destination);
    if destination_metadata
        .as_ref()
        .is_ok_and(|m| m.file_type().is_symlink())
    {
        return Err(AtomicOutputError::DestinationSymlink);
    }
    if let Some(input) = input {
        reject_collision(input, destination, destination_metadata.as_ref().ok())?;
    }
    if options.overwrite == OverwritePolicy::Forbid && destination_metadata.is_ok() {
        return Err(AtomicOutputError::DestinationExists);
    }

    let permissions = resolve_permissions(options.permissions, destination_metadata.as_ref().ok())?;
    let (temporary_path, mut temporary) = create_temporary(parent)?;
    let mut guard = TemporaryGuard(Some(temporary_path.clone()));
    writer(&mut temporary).map_err(AtomicOutputError::Write)?;
    temporary.flush().map_err(AtomicOutputError::Write)?;
    temporary
        .set_permissions(permissions)
        .map_err(AtomicOutputError::Write)?;
    temporary.sync_all().map_err(AtomicOutputError::Sync)?;
    drop(temporary);

    match options.overwrite {
        OverwritePolicy::Forbid => {
            fs::hard_link(&temporary_path, destination).map_err(|error| {
                if error.kind() == io::ErrorKind::AlreadyExists {
                    AtomicOutputError::DestinationExists
                } else {
                    AtomicOutputError::Publish(error)
                }
            })?;
            fs::remove_file(&temporary_path).map_err(AtomicOutputError::Cleanup)?;
        }
        OverwritePolicy::Replace => {
            // Re-check the final component so an attacker cannot turn it into
            // a symlink between validation and publication.
            if fs::symlink_metadata(destination)
                .as_ref()
                .is_ok_and(|m| m.file_type().is_symlink())
            {
                return Err(AtomicOutputError::DestinationSymlink);
            }
            fs::rename(&temporary_path, destination).map_err(AtomicOutputError::Publish)?;
        }
    }
    guard.0 = None;
    File::open(parent)
        .and_then(|directory| directory.sync_all())
        .map_err(AtomicOutputError::Sync)
}

fn create_temporary(parent: &Path) -> Result<(PathBuf, File), AtomicOutputError> {
    for _ in 0..128 {
        let sequence = TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let path = parent.join(format!(
            ".grainroom-output-{}-{sequence}.tmp",
            std::process::id()
        ));
        let mut options = OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        options.mode(0o600);
        match options.open(&path) {
            Ok(file) => return Ok((path, file)),
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(AtomicOutputError::Create(error)),
        }
    }
    Err(AtomicOutputError::Create(io::Error::new(
        io::ErrorKind::AlreadyExists,
        "temporary output name exhausted",
    )))
}

fn reject_collision(
    input: &Path,
    destination: &Path,
    destination_metadata: Option<&fs::Metadata>,
) -> Result<(), AtomicOutputError> {
    #[cfg(unix)]
    if let (Ok(input_metadata), Some(output_metadata)) = (fs::metadata(input), destination_metadata)
        && input_metadata.dev() == output_metadata.dev()
        && input_metadata.ino() == output_metadata.ino()
    {
        return Err(AtomicOutputError::InputOutputCollision);
    }
    let input = fs::canonicalize(input).map_err(AtomicOutputError::Create)?;
    let output_parent = destination
        .parent()
        .and_then(|path| fs::canonicalize(path).ok())
        .ok_or(AtomicOutputError::InvalidDestination)?;
    if output_parent.join(destination.file_name().unwrap()) == input {
        return Err(AtomicOutputError::InputOutputCollision);
    }
    Ok(())
}

fn resolve_permissions(
    policy: OutputPermissions,
    existing: Option<&fs::Metadata>,
) -> Result<Permissions, AtomicOutputError> {
    #[cfg(unix)]
    {
        let mode = match policy {
            OutputPermissions::Private => 0o600,
            OutputPermissions::PreserveExisting => existing.map_or(0o600, |m| m.mode() & 0o777),
            OutputPermissions::Mode(mode) if mode <= 0o777 => mode,
            OutputPermissions::Mode(_) => return Err(AtomicOutputError::InvalidPermissions),
        };
        Ok(Permissions::from_mode(mode))
    }
    #[cfg(not(unix))]
    {
        let _ = (policy, existing);
        let mut permissions = fs::metadata(".")
            .map_err(AtomicOutputError::Create)?
            .permissions();
        permissions.set_readonly(false);
        Ok(permissions)
    }
}

struct TemporaryGuard(Option<PathBuf>);
impl Drop for TemporaryGuard {
    fn drop(&mut self) {
        if let Some(path) = self.0.take() {
            let _ = fs::remove_file(path);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Read;
    use tempfile::tempdir;

    fn contents(path: &Path) -> Vec<u8> {
        let mut v = Vec::new();
        File::open(path).unwrap().read_to_end(&mut v).unwrap();
        v
    }

    #[test]
    fn success_is_private_and_complete() {
        let d = tempdir().unwrap();
        let out = d.path().join("out");
        write_atomic_output(&out, None, Default::default(), |f| f.write_all(b"complete")).unwrap();
        assert_eq!(contents(&out), b"complete");
        #[cfg(unix)]
        assert_eq!(fs::metadata(out).unwrap().mode() & 0o777, 0o600);
    }
    #[test]
    fn writer_failure_leaves_no_output_or_temporary() {
        let d = tempdir().unwrap();
        let out = d.path().join("out");
        let e = write_atomic_output(&out, None, Default::default(), |f| {
            f.write_all(b"partial")?;
            Err(io::Error::other("synthetic"))
        });
        assert!(matches!(e, Err(AtomicOutputError::Write(_))));
        assert!(!out.exists());
        assert_eq!(fs::read_dir(d.path()).unwrap().count(), 0);
    }
    #[test]
    fn forbid_never_replaces_existing() {
        let d = tempdir().unwrap();
        let out = d.path().join("out");
        fs::write(&out, b"old").unwrap();
        assert!(matches!(
            write_atomic_output(&out, None, Default::default(), |f| f.write_all(b"new")),
            Err(AtomicOutputError::DestinationExists)
        ));
        assert_eq!(contents(&out), b"old");
    }
    #[cfg(unix)]
    #[test]
    fn destination_symlink_is_rejected() {
        use std::os::unix::fs::symlink;
        let d = tempdir().unwrap();
        let target = d.path().join("target");
        fs::write(&target, b"old").unwrap();
        let out = d.path().join("out");
        symlink(&target, &out).unwrap();
        assert!(matches!(
            write_atomic_output(
                &out,
                None,
                AtomicOutputOptions {
                    overwrite: OverwritePolicy::Replace,
                    permissions: OutputPermissions::Private
                },
                |f| f.write_all(b"new")
            ),
            Err(AtomicOutputError::DestinationSymlink)
        ));
        assert_eq!(contents(&target), b"old");
    }
    #[test]
    fn input_output_collision_is_rejected() {
        let d = tempdir().unwrap();
        let input = d.path().join("same");
        fs::write(&input, b"source").unwrap();
        assert!(matches!(
            write_atomic_output(
                &input,
                Some(&input),
                AtomicOutputOptions {
                    overwrite: OverwritePolicy::Replace,
                    permissions: OutputPermissions::Private
                },
                |f| f.write_all(b"new")
            ),
            Err(AtomicOutputError::InputOutputCollision)
        ));
    }
    #[cfg(unix)]
    #[test]
    fn replacement_can_preserve_permissions() {
        let d = tempdir().unwrap();
        let out = d.path().join("out");
        fs::write(&out, b"old").unwrap();
        fs::set_permissions(&out, Permissions::from_mode(0o640)).unwrap();
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
        assert_eq!(fs::metadata(out).unwrap().mode() & 0o777, 0o640);
    }
}
