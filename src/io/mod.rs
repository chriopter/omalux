//! Codec-independent production image-I/O contracts.
//!
//! It defines bounded, auditable values exchanged with the develop pipeline,
//! production decoders, and the display-referred JPEG encoder.

mod atomic;
pub mod color;
mod digest;
pub mod encode;
mod error;
mod limits;
pub mod raster;
pub mod raw;
mod types;

pub use atomic::{
    AtomicOutputOptions, AtomicOutputOutcome, OutputPermissions, OverwritePolicy,
    write_atomic_output,
};
pub use digest::SourceDigestV1;
pub use encode::{
    EncodeCancellation, JpegEncodeInput, JpegEncodeReport, JpegEncodeRequest, MetadataWriteReport,
    PreparedDisplayRgb, encode_jpeg, prepare_display_rgb8,
};
pub use error::{
    AtomicOutputError, DecodeError, DigestError, EncodeError, ErrorCode, LimitError, MetadataKind,
    StableErrorCode,
};
pub use limits::{
    DecodeWorkingSetProfile, EncodeWorkingSetEstimate, EncodeWorkingSetProfile, ResourceLimits,
    WorkingSetEstimate,
};
pub use types::{
    AlphaPolicy, AssumedProfileReason, ColorProvenance, DecodeOptions, DecodedPhoto,
    DecodedPhotoError, Diagnostic, DiagnosticCode, DiagnosticSeverity, EncodeOptions,
    IccProfileProvenance, MetadataBundle, MetadataPolicy, OutputFormat, OutputProfile,
    PngChrmFields, PngCicpFields, PngColorDeclarationsProvenance, PngSelectedColorSource,
    RawBackendName, RawDecodeOptions, RawMatrixSource, RawProcessingProvenance, SdrRangePolicy,
    SignalRelation, UnprofiledPolicy, WhiteBalancePolicy, WhiteBalanceProvenance,
};
