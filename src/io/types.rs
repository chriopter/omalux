use super::{EncodeError, LimitError, MetadataKind, ResourceLimits, SourceDigestV1};
use crate::develop::CpuImage;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum SignalRelation {
    SceneReferred,
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
pub struct IccProfileProvenance {
    pub sha256: [u8; 32],
    pub bytes: u64,
    pub lcms_version: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PngCicpFields {
    pub color_primaries: u8,
    pub transfer_function: u8,
    pub matrix_coefficients: u8,
    pub video_full_range: bool,
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
}

#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub struct DecodedPhoto {
    pub image: CpuImage,
    pub metadata: MetadataBundle,
    pub source_digest: SourceDigestV1,
    pub color: ColorProvenance,
    pub signal_relation: SignalRelation,
    pub diagnostics: Vec<Diagnostic>,
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
}
