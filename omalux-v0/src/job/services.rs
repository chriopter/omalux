use std::{fmt, fs::File, path::Path};

use crate::{
    io::{
        DecodeOptions, DecodedPhoto, EncodeOptions, ErrorCode, OverwritePolicy, SourceFileIdentity,
        StableErrorCode,
    },
    job::{CancellationToken, DisplayReferred, EncodeSummary, WorkingArtifact},
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EncodeReceipt {
    pub bytes_written: u64,
    pub publication: PublicationStatus,
    /// Path-free codec provenance for the stable job report. Test doubles may
    /// omit it; production encoders always provide it.
    pub summary: Option<EncodeSummary>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PublicationStatus {
    PublishedAndDurable,
    /// The destination is already visible but its directory sync failed.
    /// Callers must report this state and must not retry blindly.
    PublishedButNotDurable,
}

pub struct DecodedSource {
    pub photo: DecodedPhoto,
    source: SourceLease,
}

impl DecodedSource {
    pub fn from_held_file(
        photo: DecodedPhoto,
        source: File,
    ) -> Result<Self, crate::io::AtomicOutputError> {
        Ok(Self {
            photo,
            source: SourceLease::new(source)?,
        })
    }

    pub const fn source_identity(&self) -> SourceFileIdentity {
        self.source.identity()
    }

    pub(crate) fn into_parts(self) -> (DecodedPhoto, SourceLease) {
        (self.photo, self.source)
    }
}

/// Held descriptor and identity for the exact object decoded. Keeping this
/// lease alive through publication prevents inode reuse from weakening the
/// source/destination collision check.
pub struct SourceLease {
    _file: File,
    identity: SourceFileIdentity,
}

impl SourceLease {
    fn new(file: File) -> Result<Self, crate::io::AtomicOutputError> {
        let identity = SourceFileIdentity::from_file(&file)?;
        Ok(Self {
            _file: file,
            identity,
        })
    }

    pub const fn identity(&self) -> SourceFileIdentity {
        self.identity
    }

    #[cfg(test)]
    pub(crate) fn held_file(&self) -> &File {
        &self._file
    }
}

impl fmt::Debug for SourceLease {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SourceLease")
            .field("identity", &self.identity)
            .finish_non_exhaustive()
    }
}

#[derive(Clone, Copy, Debug)]
pub struct PublicationRequest<'a> {
    pub destination: &'a Path,
    pub source: &'a SourceLease,
    pub overwrite: OverwritePolicy,
}

/// Decoder boundary for the Qt-free runner.
///
/// A production implementation must open `input` exactly once with its own
/// NOFOLLOW/regular-file policy, and derive `DecodedPhoto::source_digest` from
/// those same bytes. A path-based trait cannot mechanically enforce that
/// invariant; the future unified decoder owns and tests it.
pub trait PhotoDecoder {
    type Error: StableErrorCode;

    fn decode_path_once(
        &self,
        input: &Path,
        options: &DecodeOptions,
        cancellation: &CancellationToken,
    ) -> Result<DecodedSource, Self::Error>;
}

/// Encoder boundary accepts display-referred artifacts only.
///
/// A production implementation must publish through the atomic output API;
/// cancellation or failure must not expose a partial destination.
pub trait PhotoEncoder {
    type Error: StableErrorCode;

    fn encode_display(
        &self,
        publication: PublicationRequest<'_>,
        artifact: &WorkingArtifact<DisplayReferred>,
        options: &EncodeOptions,
        cancellation: &CancellationToken,
    ) -> Result<EncodeReceipt, Self::Error>;
}

pub(crate) fn stable_code(error: &impl StableErrorCode) -> ErrorCode {
    error.error_code()
}
