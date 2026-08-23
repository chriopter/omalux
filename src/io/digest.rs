use super::{DigestError, LimitError, ResourceLimits};
use crate::develop::DevelopRenderContext;
use sha2::{Digest, Sha256};
use std::{fs::File, io::Read, path::Path};

const DOMAIN: &[u8] = b"io.omacom.grainroom/source-digest/v1\0";

/// Versioned content identity. File names and paths never enter the hash.
#[derive(Clone, Copy, Eq, Hash, PartialEq)]
pub struct SourceDigestV1([u8; 32]);

impl SourceDigestV1 {
    /// Copies and hashes exactly the same bounded byte stream in one pass.
    pub(crate) fn copy_from_reader(
        mut reader: impl Read,
        mut writer: impl std::io::Write,
        limits: &ResourceLimits,
        mut cancelled: impl FnMut() -> bool,
    ) -> Result<(Self, u64), DigestError> {
        limits.validate().map_err(DigestError::Limit)?;
        let mut hash = Sha256::new();
        hash.update(DOMAIN);
        let mut buffer = [0_u8; 64 * 1024];
        let mut total = 0_u64;
        loop {
            if cancelled() {
                return Err(DigestError::Read(std::io::Error::new(
                    std::io::ErrorKind::Interrupted,
                    "cancelled",
                )));
            }
            let remaining = limits.max_source_bytes.saturating_sub(total);
            let request = usize::try_from(remaining.saturating_add(1))
                .unwrap_or(usize::MAX)
                .min(buffer.len());
            let count = reader
                .read(&mut buffer[..request])
                .map_err(DigestError::Read)?;
            if count == 0 {
                break;
            }
            total = total
                .checked_add(count as u64)
                .ok_or(DigestError::Limit(LimitError::ArithmeticOverflow))?;
            limits
                .check_source_bytes(total)
                .map_err(DigestError::Limit)?;
            writer
                .write_all(&buffer[..count])
                .map_err(DigestError::Read)?;
            hash.update(&buffer[..count]);
        }
        Ok((Self(hash.finalize().into()), total))
    }
    /// Streams and hashes at most `limits.max_source_bytes` bytes.
    pub fn from_reader(
        mut reader: impl Read,
        limits: &ResourceLimits,
    ) -> Result<Self, DigestError> {
        limits.validate().map_err(DigestError::Limit)?;
        let mut hash = Sha256::new();
        hash.update(DOMAIN);
        let mut buffer = [0_u8; 64 * 1024];
        let mut total = 0_u64;
        loop {
            let remaining = limits.max_source_bytes.saturating_sub(total);
            let request = usize::try_from(remaining.saturating_add(1))
                .unwrap_or(usize::MAX)
                .min(buffer.len());
            let count = reader
                .read(&mut buffer[..request])
                .map_err(DigestError::Read)?;
            if count == 0 {
                break;
            }
            total = total
                .checked_add(count as u64)
                .ok_or(DigestError::Limit(LimitError::ArithmeticOverflow))?;
            limits
                .check_source_bytes(total)
                .map_err(DigestError::Limit)?;
            hash.update(&buffer[..count]);
        }
        Ok(Self(hash.finalize().into()))
    }
    pub fn from_bytes(bytes: &[u8]) -> Self {
        let limits = ResourceLimits {
            max_source_bytes: u64::try_from(bytes.len()).unwrap_or(u64::MAX).max(1),
            ..Default::default()
        };
        Self::from_reader(bytes, &limits).expect("in-memory byte length is its own bound")
    }
    pub fn from_path(path: impl AsRef<Path>, limits: &ResourceLimits) -> Result<Self, DigestError> {
        let file = File::open(path).map_err(DigestError::Read)?;
        limits
            .check_source_bytes(file.metadata().map_err(DigestError::Read)?.len())
            .map_err(DigestError::Limit)?;
        Self::from_reader(file, limits)
    }
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
    /// Domain separation for grain and future render identities remains owned
    /// by `DevelopRenderContext`; the source digest is passed verbatim.
    pub fn develop_render_context(self) -> DevelopRenderContext {
        DevelopRenderContext::from_source_digest(self.0)
    }
}

impl std::fmt::Debug for SourceDigestV1 {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("SourceDigestV1(..)")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;
    #[test]
    fn bytes_are_content_sensitive_and_streaming_stable() {
        let a = SourceDigestV1::from_bytes(b"abc");
        let b = SourceDigestV1::from_reader(&b"abc"[..], &ResourceLimits::default()).unwrap();
        assert_eq!(a, b);
        assert_ne!(a, SourceDigestV1::from_bytes(b"abd"));
        assert_eq!(
            a.0,
            [
                0x56, 0xa4, 0x47, 0x4d, 0xfa, 0x9a, 0x99, 0x5f, 0x74, 0x6d, 0x7f, 0xc9, 0xeb, 0xe9,
                0x73, 0x45, 0xc9, 0xde, 0xb5, 0xd1, 0xe0, 0x5a, 0xa8, 0x42, 0xc2, 0x6e, 0xea, 0x6c,
                0xab, 0x6e, 0x9f, 0xb3,
            ]
        );
    }

    #[test]
    fn path_digest_is_rename_invariant_and_contains_no_path_identity() {
        let directory = tempdir().unwrap();
        let first = directory.path().join("first.bin");
        let second = directory.path().join("second.bin");
        fs::write(&first, b"same content").unwrap();
        let before = SourceDigestV1::from_path(&first, &ResourceLimits::default()).unwrap();
        fs::rename(&first, &second).unwrap();
        let after = SourceDigestV1::from_path(&second, &ResourceLimits::default()).unwrap();
        assert_eq!(before, after);
        fs::write(&second, b"changed content").unwrap();
        assert_ne!(
            before,
            SourceDigestV1::from_path(&second, &ResourceLimits::default()).unwrap()
        );
    }
    #[test]
    fn reader_and_path_reject_source_over_limit() {
        let limits = ResourceLimits {
            max_source_bytes: 2,
            ..Default::default()
        };
        assert!(matches!(
            SourceDigestV1::from_reader(&b"abc"[..], &limits),
            Err(DigestError::Limit(LimitError::SourceBytes { .. }))
        ));
        let directory = tempdir().unwrap();
        let path = directory.path().join("large");
        fs::write(&path, b"abc").unwrap();
        assert!(matches!(
            SourceDigestV1::from_path(path, &limits),
            Err(DigestError::Limit(LimitError::SourceBytes { .. }))
        ));
    }
}
