use lcms2::{CIExyY, CIExyYTRIPLE, ColorSpaceSignature, Intent, Profile, ToneCurve};
use sha2::{Digest, Sha256};
use std::fmt;

use super::ColorError;
use crate::io::{
    AssumedProfileReason, ColorProvenance, Diagnostic, DiagnosticCode, DiagnosticSeverity,
    MetadataKind, ResourceLimits,
};

const D65: CIExyY = CIExyY {
    x: 0.3127,
    y: 0.3290,
    Y: 1.0,
};
const REC2020_PRIMARIES: CIExyYTRIPLE = CIExyYTRIPLE {
    Red: CIExyY {
        x: 0.708,
        y: 0.292,
        Y: 1.0,
    },
    Green: CIExyY {
        x: 0.170,
        y: 0.797,
        Y: 1.0,
    },
    Blue: CIExyY {
        x: 0.131,
        y: 0.046,
        Y: 1.0,
    },
};

/// An opaque, validated RGB ICC profile owned by the color pipeline.
pub struct RgbProfile {
    pub(super) inner: Profile,
}

impl fmt::Debug for RgbProfile {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RgbProfile")
            .field("matrix_shaper", &self.is_matrix_shaper())
            .finish_non_exhaustive()
    }
}

impl RgbProfile {
    /// Opens bounded ICC bytes and rejects non-RGB or unusable input profiles.
    pub fn from_icc(bytes: &[u8], limits: &ResourceLimits) -> Result<Self, ColorError> {
        if bytes.is_empty() {
            return Err(ColorError::EmptyProfile);
        }
        limits.check_metadata_component(MetadataKind::Icc, bytes.len())?;
        if bytes.len() > u32::MAX as usize {
            return Err(crate::io::LimitError::MetadataBytes {
                kind: MetadataKind::Icc,
                requested: u64::try_from(bytes.len()).unwrap_or(u64::MAX),
                maximum: u64::from(u32::MAX),
            }
            .into());
        }
        let profile = Profile::new_icc(bytes).map_err(|_| ColorError::MalformedProfile)?;
        Self::validate_input(profile)
    }

    pub(super) fn from_generated(profile: Profile) -> Result<Self, ColorError> {
        if profile.color_space() != ColorSpaceSignature::RgbData {
            return Err(ColorError::ProfileGeneration);
        }
        Ok(Self { inner: profile })
    }

    fn validate_input(profile: Profile) -> Result<Self, ColorError> {
        if profile.color_space() != ColorSpaceSignature::RgbData {
            return Err(ColorError::UnsupportedColorSpace);
        }
        if !profile.is_intent_supported(Intent::RelativeColorimetric, 0) {
            return Err(ColorError::UnsupportedProfile);
        }
        Ok(Self { inner: profile })
    }

    /// Serializes the profile while enforcing the configured ICC byte limit.
    pub fn to_icc(&self, limits: &ResourceLimits) -> Result<Vec<u8>, ColorError> {
        let bytes = self
            .inner
            .icc()
            .map_err(|_| ColorError::ProfileGeneration)?;
        limits.check_metadata_component(MetadataKind::Icc, bytes.len())?;
        Ok(bytes)
    }

    /// Reports whether LCMS recognizes the profile as a matrix/TRC profile.
    /// `false` profiles remain supported when LCMS can build the input transform.
    pub fn is_matrix_shaper(&self) -> bool {
        self.inner.is_matrix_shaper()
    }
}

/// A generated standard sRGB display profile.
pub fn srgb_profile() -> RgbProfile {
    RgbProfile {
        inner: Profile::new_srgb(),
    }
}

/// A generated scene-linear Rec.2020/D65 floating-point working profile.
pub fn linear_rec2020_profile() -> Result<RgbProfile, ColorError> {
    let linear = ToneCurve::new(1.0);
    let curves = [&linear, &linear, &linear];
    let profile = Profile::new_rgb(&D65, &REC2020_PRIMARIES, &curves)
        .map_err(|_| ColorError::ProfileGeneration)?;
    RgbProfile::from_generated(profile)
}

/// An explicitly resolved source profile and its audit metadata.
pub struct ResolvedInputProfile {
    pub profile: RgbProfile,
    pub provenance: ColorProvenance,
    pub diagnostics: Vec<Diagnostic>,
}

/// Opens an embedded profile and records a content hash in provenance.
pub fn embedded_rgb_profile(
    bytes: &[u8],
    limits: &ResourceLimits,
) -> Result<ResolvedInputProfile, ColorError> {
    let profile = RgbProfile::from_icc(bytes, limits)?;
    let digest: [u8; 32] = Sha256::digest(bytes).into();
    Ok(ResolvedInputProfile {
        profile,
        provenance: ColorProvenance::EmbeddedIcc {
            profile_sha256: digest,
            profile_bytes: u64::try_from(bytes.len()).expect("ICC length was bounded to u32"),
        },
        diagnostics: Vec::new(),
    })
}

/// Makes an sRGB assumption explicit; it never silently substitutes a profile.
pub fn assumed_srgb_profile(reason: AssumedProfileReason) -> ResolvedInputProfile {
    let code = match reason {
        AssumedProfileReason::MissingProfile => DiagnosticCode::MissingProfileAssumedSrgb,
        AssumedProfileReason::UnsupportedProfile => DiagnosticCode::UnsupportedProfileAssumedSrgb,
    };
    ResolvedInputProfile {
        profile: srgb_profile(),
        provenance: ColorProvenance::AssumedSrgb { reason },
        diagnostics: vec![Diagnostic {
            severity: DiagnosticSeverity::Warning,
            code,
        }],
    }
}

/// Runtime LCMS version encoded as `major * 1000 + minor * 10 + patch`.
pub fn lcms_version() -> u32 {
    lcms2::version()
}

#[cfg(test)]
mod tests {
    use super::*;
    use lcms2::ColorSpaceSignature;

    #[test]
    fn generated_profiles_are_bounded_rgb_and_roundtrip_through_icc() {
        let limits = ResourceLimits::default();
        for profile in [srgb_profile(), linear_rec2020_profile().unwrap()] {
            let bytes = profile.to_icc(&limits).unwrap();
            let reopened = RgbProfile::from_icc(&bytes, &limits).unwrap();
            assert!(reopened.is_matrix_shaper());
        }
    }

    #[test]
    fn malformed_truncated_oversized_and_non_rgb_profiles_are_rejected() {
        let limits = ResourceLimits::default();
        assert_eq!(
            RgbProfile::from_icc(&[], &limits).unwrap_err(),
            ColorError::EmptyProfile
        );
        assert_eq!(
            RgbProfile::from_icc(&[0; 128], &limits).unwrap_err(),
            ColorError::MalformedProfile
        );

        let srgb = srgb_profile().to_icc(&limits).unwrap();
        let tiny_limit = ResourceLimits {
            max_icc_bytes: 16,
            ..limits
        };
        assert!(matches!(
            RgbProfile::from_icc(&srgb, &tiny_limit),
            Err(ColorError::Limit(crate::io::LimitError::MetadataBytes {
                kind: MetadataKind::Icc,
                ..
            }))
        ));

        let linear = ToneCurve::new(1.0);
        let gray = Profile::new_gray(&D65, &linear).unwrap().icc().unwrap();
        assert_eq!(
            RgbProfile::from_icc(&gray, &limits).unwrap_err(),
            ColorError::UnsupportedColorSpace
        );

        let cmyk = Profile::ink_limiting(ColorSpaceSignature::CmykData, 300.0)
            .unwrap()
            .icc()
            .unwrap();
        assert_eq!(
            RgbProfile::from_icc(&cmyk, &limits).unwrap_err(),
            ColorError::UnsupportedColorSpace
        );
    }

    #[test]
    fn assumptions_are_never_silent() {
        let assumed = assumed_srgb_profile(AssumedProfileReason::MissingProfile);
        assert_eq!(
            assumed.provenance,
            ColorProvenance::AssumedSrgb {
                reason: AssumedProfileReason::MissingProfile
            }
        );
        assert_eq!(
            assumed.diagnostics,
            vec![Diagnostic {
                severity: DiagnosticSeverity::Warning,
                code: DiagnosticCode::MissingProfileAssumedSrgb,
            }]
        );
    }

    #[test]
    fn runtime_lcms_version_is_the_expected_supported_family() {
        // LCMS encodes 2.19 as 2190. Keep this broad enough for compatible
        // system patch updates while detecting a wrong major library.
        assert!((2000..3000).contains(&lcms_version()));
    }
}
