#[cfg(unix)]
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::{
    fs::{File, OpenOptions},
    io,
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

use crate::io::{DecodeError, DigestError, ResourceLimits, SourceDigestV1};

pub(super) struct StagedRaw {
    path: PathBuf,
    pub digest: SourceDigestV1,
}
impl StagedRaw {
    pub fn path(&self) -> &Path {
        &self.path
    }
}
impl Drop for StagedRaw {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}

pub(super) fn stage_source(
    source: &Path,
    directory: &Path,
    limits: &ResourceLimits,
    cancelled: &Arc<AtomicBool>,
) -> Result<StagedRaw, DecodeError> {
    if cancelled.load(Ordering::Acquire) {
        return Err(DecodeError::Cancelled);
    }
    let mut input = File::open(source).map_err(DecodeError::Input)?;
    if !input.metadata().map_err(DecodeError::Input)?.is_file() {
        return Err(DecodeError::UnsupportedFormat);
    }
    let suffix = safe_suffix(source);
    std::fs::create_dir_all(directory).map_err(DecodeError::Input)?;
    for _ in 0..16 {
        let path = directory.join(format!(".grainroom-raw-{}.{}", random_hex()?, suffix));
        let mut options = OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        options.mode(0o600);
        let mut output = match options.open(&path) {
            Ok(file) => file,
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(DecodeError::Input(error)),
        };
        let result = SourceDigestV1::copy_from_reader(&mut input, &mut output, limits, || {
            cancelled.load(Ordering::Acquire)
        });
        match result {
            Ok((digest, _)) => {
                output.sync_all().map_err(DecodeError::Input)?;
                #[cfg(unix)]
                if output
                    .metadata()
                    .map_err(DecodeError::Input)?
                    .permissions()
                    .mode()
                    & 0o777
                    != 0o600
                {
                    let _ = std::fs::remove_file(&path);
                    return Err(DecodeError::Input(io::Error::new(
                        io::ErrorKind::PermissionDenied,
                        "staged raw is not private",
                    )));
                }
                drop(output);
                return Ok(StagedRaw { path, digest });
            }
            Err(error) => {
                drop(output);
                let _ = std::fs::remove_file(&path);
                return Err(map_digest(error));
            }
        }
    }
    Err(DecodeError::Input(io::Error::new(
        io::ErrorKind::AlreadyExists,
        "secure staged names exhausted",
    )))
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
    use tempfile::tempdir;
    #[test]
    fn suffix_is_safe() {
        assert_eq!(safe_suffix(Path::new("x.NEF")), "nef");
        assert_eq!(safe_suffix(Path::new("x.$(bad)")), "raw");
    }
    #[test]
    fn staging_is_private_bounded_and_cleans() {
        let d = tempdir().unwrap();
        let source = d.path().join("source.nef");
        std::fs::write(&source, b"abc").unwrap();
        let stage_dir = d.path().join("stage");
        let staged = stage_source(
            &source,
            &stage_dir,
            &ResourceLimits::default(),
            &Arc::new(AtomicBool::new(false)),
        )
        .unwrap();
        let path = staged.path.clone();
        assert_eq!(std::fs::read(&path).unwrap(), b"abc");
        drop(staged);
        assert!(!path.exists());
    }
}
