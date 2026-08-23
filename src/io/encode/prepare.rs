use crate::{
    develop::CpuImage,
    io::{EncodeError, EncodeOptions, MetadataBundle, ResourceLimits, SignalRelation},
};

use super::{EncodeCancellation, MetadataWriteReport};

pub struct JpegEncodeInput<'a> {
    pub image: &'a CpuImage,
    pub signal_relation: SignalRelation,
    pub metadata: &'a MetadataBundle,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PreparedDisplayRgb {
    pub width: u32,
    pub height: u32,
    pub rgb8: Vec<u8>,
    pub icc: Vec<u8>,
    pub icc_provenance: crate::io::IccProfileProvenance,
    pub clipped_samples: u64,
    pub alpha_flattened_pixels: u64,
    pub exif: Option<Vec<u8>>,
    pub metadata_report: MetadataWriteReport,
}

pub fn prepare_display_rgb8(
    input: JpegEncodeInput<'_>,
    options: &EncodeOptions,
    limits: &ResourceLimits,
    cancellation: &EncodeCancellation,
) -> Result<PreparedDisplayRgb, EncodeError> {
    let _ = (input, options, limits, cancellation);
    Err(EncodeError::Encode)
}
