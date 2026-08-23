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
    SourceFileIdentity, write_atomic_output, write_atomic_output_for_source,
};
pub use digest::SourceDigestV1;
pub use encode::{
    EncodeCancellation, HeicCapability, HeicEncodeReport, HeicEncodeRequest, JpegEncodeInput,
    JpegEncodeReport, JpegEncodeRequest, MetadataWriteReport, PreparedDisplayRgb, encode_heic,
    encode_jpeg, prepare_display_rgb8, probe_heic_capability,
};
pub use error::{
    AtomicOutputError, DecodeError, DigestError, EncodeError, ErrorCode, LimitError, MetadataKind,
    StableErrorCode,
};
pub use limits::{
    DecodeWorkingSetProfile, EncodeWorkingSetEstimate, EncodeWorkingSetProfile,
    JpegMetadataFootprint, ResourceLimits, WorkingSetEstimate,
};
pub use types::{
    AlphaPolicy, AssumedProfileReason, ColorProvenance, DecodeOptions, DecodedPhoto,
    DecodedPhotoError, Diagnostic, DiagnosticCode, DiagnosticSeverity, EncodeOptions,
    IccProfileProvenance, MetadataBundle, MetadataPolicy, OutputFormat, OutputProfile,
    PngChrmFields, PngCicpFields, PngColorDeclarationsProvenance, PngSelectedColorSource,
    RawBackendName, RawDecodeOptions, RawMatrixSource, RawProcessingProvenance, SdrRangePolicy,
    SignalRelation, UnprofiledPolicy, WhiteBalancePolicy, WhiteBalanceProvenance,
};
