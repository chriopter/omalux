use std::path::Path;

use crate::{
    io::{
        DecodeOptions, DecodedPhoto, EncodeOptions, ErrorCode, OverwritePolicy, SourceFileIdentity,
        StableErrorCode,
    },
    job::{CancellationToken, DisplayReferred, WorkingArtifact},
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct EncodeReceipt {
    pub bytes_written: u64,
    pub publication: PublicationStatus,
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
    pub source_identity: SourceFileIdentity,
}

#[derive(Clone, Copy, Debug)]
pub struct PublicationRequest<'a> {
    pub destination: &'a Path,
    pub source_identity: SourceFileIdentity,
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
