use crate::io::{EncodeError, MetadataBundle, MetadataPolicy, ResourceLimits};

#[derive(Clone, Debug, Default, Eq, PartialEq)]
#[non_exhaustive]
pub struct MetadataWriteReport {
    pub exif_input_bytes: u64,
    pub exif_output_bytes: u64,
    pub gps_removed: bool,
    pub orientation_removed: bool,
    pub unsafe_tags_removed: u32,
    pub xmp_dropped: bool,
    pub iptc_dropped: bool,
    pub malformed_exif_dropped: bool,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct SanitizedMetadata {
    pub exif: Option<Vec<u8>>,
    pub report: MetadataWriteReport,
}

pub(crate) fn sanitize_metadata(
    _metadata: &MetadataBundle,
    _policy: MetadataPolicy,
    _limits: &ResourceLimits,
    _cancelled: impl Fn() -> bool,
) -> Result<SanitizedMetadata, EncodeError> {
    Ok(SanitizedMetadata::default())
}
