//! Codec-independent production image-I/O contracts.
//!
//! This module intentionally contains no decoder or encoder. It defines the
//! bounded, auditable values those backends must exchange with the develop
//! pipeline and stable error classifications suitable for a future CLI.

mod atomic;
pub mod color;
mod digest;
mod error;
mod limits;
pub mod raw;
pub mod raster;
mod types;

pub use atomic::{
    AtomicOutputOptions, AtomicOutputOutcome, OutputPermissions, OverwritePolicy,
    write_atomic_output,
};
pub use digest::SourceDigestV1;
pub use error::{
    AtomicOutputError, DecodeError, DigestError, EncodeError, ErrorCode, LimitError, MetadataKind,
    StableErrorCode,
};
pub use limits::{DecodeWorkingSetProfile, ResourceLimits, WorkingSetEstimate};
pub use types::{
    AlphaPolicy, AssumedProfileReason, ColorProvenance, DecodeOptions, DecodedPhoto,
    DecodedPhotoError, Diagnostic, DiagnosticCode, DiagnosticSeverity, EncodeOptions,
    IccProfileProvenance, MetadataBundle, MetadataPolicy, OutputFormat, OutputProfile,
    PngChrmFields, PngCicpFields, PngColorDeclarationsProvenance, PngSelectedColorSource,
    RawBackendName, RawDecodeOptions, RawMatrixSource, RawProcessingProvenance, SdrRangePolicy,
    SignalRelation, UnprofiledPolicy, WhiteBalancePolicy, WhiteBalanceProvenance,
};
