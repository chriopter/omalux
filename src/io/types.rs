use super::{EncodeError, LimitError, MetadataKind, ResourceLimits, SourceDigestV1};
use crate::develop::{CpuImage, ImageError};
use std::fmt;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum SignalRelation {
    /// Linear Rec.2020 produced by the RAW scene pipeline; display rendering
    /// is still required before an SDR display encoding can be produced.
    SceneRelatedRaw,
    LinearizedDisplayReferred,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AssumedProfileReason {
    MissingProfile,
    UnsupportedProfile,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RawMatrixSource {
    FileMetadata,
    CameraDatabase,
    Unknown,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WhiteBalanceProvenance {
    Camera,
    DaylightFallback,
    Explicit,
    Unknown,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum RawBackendName {
    LibRawDcrawEmu,
}

/// Exact processing choices used to produce the working-space pixels.
#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub struct RawProcessingProvenance {
    pub backend: RawBackendName,
    /// `None` means the installed executable exposed no version interface.
    pub backend_version: Option<String>,
    pub full_resolution: bool,
    pub linear_16_bit: bool,
    pub output_rec2020: bool,
    pub embedded_matrix_enabled: bool,
    pub ahd_demosaic: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct IccProfileProvenance {
    pub sha256: [u8; 32],
    pub bytes: u64,
    pub lcms_version: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PngCicpFields {
    color_primaries: u8,
    transfer_function: u8,
    matrix_coefficients: u8,
    video_full_range_flag: u8,
}

impl PngCicpFields {
    /// Parses the four raw cICP bytes. PNG permits only full-range flag 0 or 1.
    pub fn try_from_raw(
        color_primaries: u8,
        transfer_function: u8,
        matrix_coefficients: u8,
        video_full_range_flag: u8,
    ) -> Result<Self, crate::io::color::ColorError> {
        if video_full_range_flag > 1 {
            return Err(crate::io::color::ColorError::InvalidPngCicp);
        }
        Ok(Self {
            color_primaries,
            transfer_function,
            matrix_coefficients,
            video_full_range_flag,
        })
    }

    pub const fn color_primaries(self) -> u8 {
        self.color_primaries
    }

    pub const fn transfer_function(self) -> u8 {
        self.transfer_function
    }

    pub const fn matrix_coefficients(self) -> u8 {
        self.matrix_coefficients
    }

    pub const fn video_full_range_flag(self) -> u8 {
        self.video_full_range_flag
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PngChrmFields {
    pub white_x: u32,
    pub white_y: u32,
    pub red_x: u32,
    pub red_y: u32,
    pub green_x: u32,
    pub green_y: u32,
    pub blue_x: u32,
    pub blue_y: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PngColorDeclarationsProvenance {
    pub cicp: Option<PngCicpFields>,
    pub embedded_icc: Option<IccProfileProvenance>,
    pub srgb_rendering_intent: Option<u8>,
    pub gamma_times_100000: Option<u32>,
    pub chromaticities_times_100000: Option<PngChrmFields>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PngSelectedColorSource {
    Cicp,
    EmbeddedIcc,
    Srgb,
    ChromaticitiesAndGamma,
}

/// Audit information only; it is not proof of colorimetric accuracy.
#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum ColorProvenance {
    EmbeddedIcc {
        profile_sha256: [u8; 32],
        profile_bytes: u64,
        lcms_version: u32,
    },
    DeclaredSrgb,
    PngDeclared {
        selected: PngSelectedColorSource,
        declarations: PngColorDeclarationsProvenance,
        resolved_profile: IccProfileProvenance,
    },
    AssumedSrgb {
        reason: AssumedProfileReason,
    },
    RawMatrix {
        matrix: RawMatrixSource,
        white_balance: WhiteBalanceProvenance,
        processing: RawProcessingProvenance,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum DiagnosticSeverity {
    Information,
    Warning,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum DiagnosticCode {
    MissingProfileAssumedSrgb,
    UnsupportedProfileAssumedSrgb,
    RawWhiteBalanceFallback,
    UnknownRawMatrix,
    MetadataDropped,
    OutputRangeClipped,
    CameraWhiteBalanceFallbackUnknown,
    BackendVersionUnavailable,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Diagnostic {
    pub severity: DiagnosticSeverity,
    pub code: DiagnosticCode,
}

/// Bounded metadata payload. It deliberately contains no source path or file name.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
#[non_exhaustive]
pub struct MetadataBundle {
    exif: Option<Vec<u8>>,
    xmp: Option<Vec<u8>>,
    iptc: Option<Vec<u8>>,
    orientation_consumed: bool,
}
impl MetadataBundle {
    pub fn try_new(
        exif: Option<Vec<u8>>,
        xmp: Option<Vec<u8>>,
        iptc: Option<Vec<u8>>,
        orientation_consumed: bool,
        limits: &ResourceLimits,
    ) -> Result<Self, LimitError> {
        let mut total = 0_u64;
        for (kind, value) in [
            (MetadataKind::Exif, exif.as_deref()),
            (MetadataKind::Xmp, xmp.as_deref()),
            (MetadataKind::Iptc, iptc.as_deref()),
        ] {
            if let Some(value) = value {
                limits.check_metadata_component(kind, value.len())?;
                total = total
                    .checked_add(value.len() as u64)
                    .ok_or(LimitError::ArithmeticOverflow)?;
            }
        }
        limits.check_metadata_total(total)?;
        Ok(Self {
            exif,
            xmp,
            iptc,
            orientation_consumed,
        })
    }
    pub fn exif(&self) -> Option<&[u8]> {
        self.exif.as_deref()
    }
    pub fn xmp(&self) -> Option<&[u8]> {
        self.xmp.as_deref()
    }
    pub fn iptc(&self) -> Option<&[u8]> {
        self.iptc.as_deref()
    }
    pub const fn orientation_consumed(&self) -> bool {
        self.orientation_consumed
    }

    fn validate(&self, limits: &ResourceLimits) -> Result<(), LimitError> {
        limits.validate()?;
        let mut total = 0_u64;
        for (kind, value) in [
            (MetadataKind::Exif, self.exif.as_deref()),
            (MetadataKind::Xmp, self.xmp.as_deref()),
            (MetadataKind::Iptc, self.iptc.as_deref()),
        ] {
            if let Some(value) = value {
                limits.check_metadata_component(kind, value.len())?;
                total = total
                    .checked_add(value.len() as u64)
                    .ok_or(LimitError::ArithmeticOverflow)?;
            }
        }
        limits.check_metadata_total(total)
    }
}

#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub struct DecodedPhoto {
    image: CpuImage,
    metadata: MetadataBundle,
    source_digest: SourceDigestV1,
    color: ColorProvenance,
    signal_relation: SignalRelation,
    diagnostics: Vec<Diagnostic>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum DecodedPhotoError {
    Image(ImageError),
    Limit(LimitError),
    ColorRelationMismatch,
}

impl fmt::Display for DecodedPhotoError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Image(error) => error.fmt(formatter),
            Self::Limit(error) => error.fmt(formatter),
            Self::ColorRelationMismatch => formatter
                .write_str("decoded color provenance is inconsistent with its signal relation"),
        }
    }
}

impl std::error::Error for DecodedPhotoError {}

impl DecodedPhoto {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        image: CpuImage,
        metadata: MetadataBundle,
        source_digest: SourceDigestV1,
        color: ColorProvenance,
        signal_relation: SignalRelation,
        diagnostics: Vec<Diagnostic>,
        limits: &ResourceLimits,
    ) -> Result<Self, DecodedPhotoError> {
        let photo = Self {
            image,
            metadata,
            source_digest,
            color,
            signal_relation,
            diagnostics,
        };
        photo.validate(limits)?;
        Ok(photo)
    }

    pub fn validate(&self, limits: &ResourceLimits) -> Result<(), DecodedPhotoError> {
        limits.validate().map_err(DecodedPhotoError::Limit)?;
        self.image.validate().map_err(DecodedPhotoError::Image)?;
        let pixels = u64::from(self.image.width())
            .checked_mul(u64::from(self.image.height()))
            .ok_or(DecodedPhotoError::Limit(LimitError::ArithmeticOverflow))?;
        if pixels > limits.max_pixels {
            return Err(DecodedPhotoError::Limit(LimitError::PixelCount {
                requested: pixels,
                maximum: limits.max_pixels,
            }));
        }
        let working_bytes = pixels
            .checked_mul(16)
            .ok_or(DecodedPhotoError::Limit(LimitError::ArithmeticOverflow))?;
        if working_bytes > limits.max_working_bytes {
            return Err(DecodedPhotoError::Limit(LimitError::WorkingBytes {
                requested: working_bytes,
                maximum: limits.max_working_bytes,
            }));
        }
        self.metadata
            .validate(limits)
            .map_err(DecodedPhotoError::Limit)?;
        let expected = match self.color {
            ColorProvenance::RawMatrix { .. } => Some(SignalRelation::SceneRelatedRaw),
            ColorProvenance::EmbeddedIcc { .. }
            | ColorProvenance::DeclaredSrgb
            | ColorProvenance::PngDeclared { .. }
            | ColorProvenance::AssumedSrgb { .. } => {
                Some(SignalRelation::LinearizedDisplayReferred)
            }
        };
        if expected.is_some_and(|expected| expected != self.signal_relation) {
            return Err(DecodedPhotoError::ColorRelationMismatch);
        }
        Ok(())
    }

    pub fn image(&self) -> &CpuImage {
        &self.image
    }
    pub fn metadata(&self) -> &MetadataBundle {
        &self.metadata
    }
    pub const fn source_digest(&self) -> SourceDigestV1 {
        self.source_digest
    }
    pub fn color(&self) -> &ColorProvenance {
        &self.color
    }
    pub const fn signal_relation(&self) -> SignalRelation {
        self.signal_relation
    }
    pub fn diagnostics(&self) -> &[Diagnostic] {
        &self.diagnostics
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum UnprofiledPolicy {
    AssumeSrgbAndWarn,
    Reject,
}
#[derive(Clone, Copy, Debug, PartialEq)]
#[non_exhaustive]
pub enum WhiteBalancePolicy {
    CameraThenDaylight,
    Daylight,
    Explicit([f32; 4]),
}
#[derive(Clone, Copy, Debug, PartialEq)]
#[non_exhaustive]
pub struct RawDecodeOptions {
    pub white_balance: WhiteBalancePolicy,
    pub apply_orientation: bool,
}
impl Default for RawDecodeOptions {
    fn default() -> Self {
        Self {
            white_balance: WhiteBalancePolicy::CameraThenDaylight,
            apply_orientation: true,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq)]
#[non_exhaustive]
pub struct DecodeOptions {
    pub limits: ResourceLimits,
    pub unprofiled: UnprofiledPolicy,
    pub raw: RawDecodeOptions,
}
impl Default for DecodeOptions {
    fn default() -> Self {
        Self {
            limits: Default::default(),
            unprofiled: UnprofiledPolicy::AssumeSrgbAndWarn,
            raw: Default::default(),
        }
    }
}
impl DecodeOptions {
    pub fn validate(&self) -> Result<(), super::DecodeError> {
        self.limits.validate().map_err(super::DecodeError::Limit)?;
        if let WhiteBalancePolicy::Explicit(multipliers) = self.raw.white_balance
            && !multipliers
                .iter()
                .all(|value| value.is_finite() && *value > 0.0 && *value <= 64.0)
        {
            return Err(super::DecodeError::InvalidOptions);
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum OutputFormat {
    Jpeg,
    Heic,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum OutputProfile {
    Srgb,
    DisplayP3,
    Rec2020,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum MetadataPolicy {
    PreserveSafe,
    StripLocation,
    StripAll,
}
#[derive(Clone, Copy, Debug, PartialEq)]
#[non_exhaustive]
pub enum AlphaPolicy {
    Reject,
    Flatten([f32; 3]),
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum SdrRangePolicy {
    ClipAndReport,
    Reject,
}
#[derive(Clone, Copy, Debug, PartialEq)]
#[non_exhaustive]
pub struct EncodeOptions {
    pub format: OutputFormat,
    pub quality: u8,
    pub profile: OutputProfile,
    pub metadata: MetadataPolicy,
    pub alpha: AlphaPolicy,
    pub range: SdrRangePolicy,
}
impl EncodeOptions {
    pub fn validate(&self) -> Result<(), EncodeError> {
        if !(1..=100).contains(&self.quality) {
            return Err(EncodeError::InvalidOptions);
        }
        if let AlphaPolicy::Flatten(rgb) = self.alpha
            && !rgb.iter().all(|v| v.is_finite() && (0.0..=1.0).contains(v))
        {
            return Err(EncodeError::InvalidOptions);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn metadata_is_bounded() {
        let l = ResourceLimits {
            max_metadata_component_bytes: 2,
            ..Default::default()
        };
        assert!(MetadataBundle::try_new(Some(vec![0; 3]), None, None, false, &l).is_err());
    }
    #[test]
    fn quality_and_flatten_are_validated() {
        let mut o = EncodeOptions {
            format: OutputFormat::Jpeg,
            quality: 90,
            profile: OutputProfile::Srgb,
            metadata: MetadataPolicy::StripLocation,
            alpha: AlphaPolicy::Flatten([0.0, 0.0, 0.0]),
            range: SdrRangePolicy::ClipAndReport,
        };
        assert!(o.validate().is_ok());
        o.quality = 0;
        assert!(o.validate().is_err());
    }
    #[test]
    fn explicit_white_balance_is_finite_positive_and_bounded() {
        let mut o = DecodeOptions::default();
        for bad in [0.0, -1.0, f32::NAN, f32::INFINITY, 65.0] {
            o.raw.white_balance = WhiteBalancePolicy::Explicit([1.0, bad, 1.0, 1.0]);
            assert!(o.validate().is_err());
        }
        o.raw.white_balance = WhiteBalancePolicy::Explicit([2.0, 1.0, 1.5, 1.0]);
        assert!(o.validate().is_ok());
    }

    #[test]
    fn decoded_photo_constructor_enforces_relation_and_resource_invariants() {
        let image = CpuImage::new(
            1,
            1,
            vec![crate::develop::RgbaPixel::new(0.1, 0.2, 0.3, 1.0).unwrap()],
        )
        .unwrap();
        let digest = SourceDigestV1::from_bytes(b"decoded-photo-test");
        let limits = ResourceLimits::default();
        assert!(
            DecodedPhoto::new(
                image.clone(),
                MetadataBundle::default(),
                digest,
                ColorProvenance::DeclaredSrgb,
                SignalRelation::LinearizedDisplayReferred,
                Vec::new(),
                &limits,
            )
            .is_ok()
        );
        assert_eq!(
            DecodedPhoto::new(
                image.clone(),
                MetadataBundle::default(),
                digest,
                ColorProvenance::RawMatrix {
                    matrix: RawMatrixSource::CameraDatabase,
                    white_balance: WhiteBalanceProvenance::Camera,
                },
                SignalRelation::LinearizedDisplayReferred,
                Vec::new(),
                &limits,
            )
            .unwrap_err(),
            DecodedPhotoError::ColorRelationMismatch
        );
        assert_eq!(
            DecodedPhoto::new(
                image.clone(),
                MetadataBundle::default(),
                digest,
                ColorProvenance::EmbeddedIcc {
                    profile_sha256: [7; 32],
                    profile_bytes: 128,
                    lcms_version: 2_170,
                },
                SignalRelation::SceneRelatedRaw,
                Vec::new(),
                &limits,
            )
            .unwrap_err(),
            DecodedPhotoError::ColorRelationMismatch
        );
        let constrained = ResourceLimits {
            max_working_bytes: 15,
            ..limits
        };
        assert!(matches!(
            DecodedPhoto::new(
                image,
                MetadataBundle::default(),
                digest,
                ColorProvenance::DeclaredSrgb,
                SignalRelation::LinearizedDisplayReferred,
                Vec::new(),
                &constrained,
            ),
            Err(DecodedPhotoError::Limit(LimitError::WorkingBytes { .. }))
        ));
    }
}
