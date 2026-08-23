use lcms2::{CIExyY, CIExyYTRIPLE, Profile, ToneCurve};

use super::{ColorError, PngColorChunk, RgbProfile, embedded_rgb_profile, srgb_profile};
use crate::io::{
    ColorProvenance, Diagnostic, PngChrmFields, PngCicpFields, PngColorDeclarationsProvenance,
    PngSelectedColorSource, ResourceLimits, SignalRelation,
};

const CHROMATICITY_SCALE: f64 = 100_000.0;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PngChunk<T> {
    pub value: Option<T>,
    pub duplicate: bool,
}

impl<T> PngChunk<T> {
    pub const fn absent() -> Self {
        Self {
            value: None,
            duplicate: false,
        }
    }

    pub const fn one(value: T) -> Self {
        Self {
            value: Some(value),
            duplicate: false,
        }
    }

    pub const fn duplicated(first_value: T) -> Self {
        Self {
            value: Some(first_value),
            duplicate: true,
        }
    }

    const fn is_present(&self) -> bool {
        self.value.is_some() || self.duplicate
    }
}

impl<T> Default for PngChunk<T> {
    fn default() -> Self {
        Self::absent()
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct PngColorDeclarations<'a> {
    pub cicp: PngChunk<PngCicpFields>,
    pub iccp: PngChunk<&'a [u8]>,
    pub srgb_rendering_intent: PngChunk<u8>,
    /// Raw PNG gAMA integer (`image_gamma * 100000`).
    pub gamma_times_100000: PngChunk<u32>,
    /// Raw PNG cHRM integers (`chromaticity * 100000`).
    pub chromaticities_times_100000: PngChunk<PngChrmFields>,
}

#[derive(Debug)]
pub struct SynthesizedPngProfile {
    pub profile: RgbProfile,
    pub provenance: ColorProvenance,
    pub diagnostics: Vec<Diagnostic>,
    /// Semantic relation of pixels after conversion into the working space.
    pub working_signal_relation: SignalRelation,
}

/// Resolves understood PNG color declarations by the normative priority in
/// PNG Third Edition section 4.3, Table 1:
/// cICP, iCCP, sRGB, then cHRM+gAMA.
///
/// Lower-priority chunks are ignored, including contradictory values. A
/// selected chunk is rejected only for its own duplicate/invalid/unsupported
/// semantics; no fallback occurs after a selected understood chunk fails.
pub fn resolve_png_color_declarations(
    declarations: PngColorDeclarations<'_>,
    limits: &ResourceLimits,
) -> Result<SynthesizedPngProfile, ColorError> {
    limits.validate()?;
    if declarations.cicp.is_present() {
        reject_duplicate(&declarations.cicp, PngColorChunk::Cicp)?;
        let cicp = declarations.cicp.value.ok_or(ColorError::InvalidPngCicp)?;
        let profile = cicp_profile(cicp, limits)?;
        return resolved(profile, PngSelectedColorSource::Cicp, declarations, None);
    }
    if declarations.iccp.is_present() {
        reject_duplicate(&declarations.iccp, PngColorChunk::Iccp)?;
        let bytes = declarations
            .iccp
            .value
            .ok_or(ColorError::MalformedProfile)?;
        let embedded = embedded_rgb_profile(bytes, limits)?;
        let selected_icc = Some(embedded.profile.icc_provenance());
        return resolved(
            embedded.profile,
            PngSelectedColorSource::EmbeddedIcc,
            declarations,
            selected_icc,
        );
    }
    if declarations.srgb_rendering_intent.is_present() {
        reject_duplicate(&declarations.srgb_rendering_intent, PngColorChunk::Srgb)?;
        let intent = declarations
            .srgb_rendering_intent
            .value
            .ok_or(ColorError::InvalidPngSrgbIntent)?;
        if intent > 3 {
            return Err(ColorError::InvalidPngSrgbIntent);
        }
        return resolved(
            srgb_profile(limits)?,
            PngSelectedColorSource::Srgb,
            declarations,
            None,
        );
    }
    if declarations.gamma_times_100000.is_present()
        || declarations.chromaticities_times_100000.is_present()
    {
        reject_duplicate(&declarations.gamma_times_100000, PngColorChunk::Gamma)?;
        reject_duplicate(
            &declarations.chromaticities_times_100000,
            PngColorChunk::Chromaticities,
        )?;
        let gamma = declarations
            .gamma_times_100000
            .value
            .ok_or(ColorError::IncompletePngDeclaration)?;
        let chromaticities = declarations
            .chromaticities_times_100000
            .value
            .ok_or(ColorError::IncompletePngDeclaration)?;
        let profile = gamma_chromaticity_profile(gamma, chromaticities, limits)?;
        return resolved(
            profile,
            PngSelectedColorSource::ChromaticitiesAndGamma,
            declarations,
            None,
        );
    }
    Err(ColorError::IncompletePngDeclaration)
}

fn resolved(
    profile: RgbProfile,
    selected: PngSelectedColorSource,
    declarations: PngColorDeclarations<'_>,
    selected_icc: Option<crate::io::IccProfileProvenance>,
) -> Result<SynthesizedPngProfile, ColorError> {
    let resolved_profile = profile.icc_provenance();
    Ok(SynthesizedPngProfile {
        profile,
        provenance: ColorProvenance::PngDeclared {
            selected,
            declarations: PngColorDeclarationsProvenance {
                cicp: declarations.cicp.value,
                embedded_icc: selected_icc,
                srgb_rendering_intent: declarations.srgb_rendering_intent.value,
                gamma_times_100000: declarations.gamma_times_100000.value,
                chromaticities_times_100000: declarations.chromaticities_times_100000.value,
            },
            resolved_profile,
        },
        diagnostics: Vec::new(),
        working_signal_relation: SignalRelation::LinearizedDisplayReferred,
    })
}

fn reject_duplicate<T>(chunk: &PngChunk<T>, kind: PngColorChunk) -> Result<(), ColorError> {
    if chunk.duplicate {
        Err(ColorError::DuplicatePngDeclaration(kind))
    } else {
        Ok(())
    }
}

fn cicp_profile(cicp: PngCicpFields, limits: &ResourceLimits) -> Result<RgbProfile, ColorError> {
    if cicp.matrix_coefficients != 0 {
        return Err(ColorError::InvalidPngCicp);
    }
    if matches!(cicp.transfer_function, 16 | 18) {
        return Err(ColorError::UnsupportedHdrPngCicp);
    }
    if cicp.color_primaries != 1 || !cicp.video_full_range {
        return Err(ColorError::UnsupportedPngCicp);
    }
    match cicp.transfer_function {
        13 => srgb_profile(limits),
        1 => bt709_profile(limits),
        _ => Err(ColorError::UnsupportedPngCicp),
    }
}

fn bt709_profile(limits: &ResourceLimits) -> Result<RgbProfile, ColorError> {
    let transfer = ToneCurve::new_parametric(
        4,
        &[1.0 / 0.45, 1.0 / 1.099, 0.099 / 1.099, 1.0 / 4.5, 0.081],
    )
    .map_err(|_| ColorError::ProfileGeneration)?;
    rgb_profile(PngChrmFields::SRGB, transfer, limits)
}

fn gamma_chromaticity_profile(
    gamma_raw: u32,
    chromaticities: PngChrmFields,
    limits: &ResourceLimits,
) -> Result<RgbProfile, ColorError> {
    if gamma_raw == 0 {
        return Err(ColorError::InvalidPngGamma);
    }
    let png_gamma = f64::from(gamma_raw) / CHROMATICITY_SCALE;
    let decoding_exponent = png_gamma.recip();
    if !png_gamma.is_finite() || png_gamma < f64::MIN_POSITIVE || !decoding_exponent.is_finite() {
        return Err(ColorError::InvalidPngGamma);
    }
    validate_chromaticities(chromaticities)?;
    rgb_profile(chromaticities, ToneCurve::new(decoding_exponent), limits)
}

fn rgb_profile(
    chromaticities: PngChrmFields,
    transfer: ToneCurve,
    limits: &ResourceLimits,
) -> Result<RgbProfile, ColorError> {
    let curves = [&transfer, &transfer, &transfer];
    let white = xyy(chromaticities.white_x, chromaticities.white_y);
    let primaries = CIExyYTRIPLE {
        Red: xyy(chromaticities.red_x, chromaticities.red_y),
        Green: xyy(chromaticities.green_x, chromaticities.green_y),
        Blue: xyy(chromaticities.blue_x, chromaticities.blue_y),
    };
    let profile =
        Profile::new_rgb(&white, &primaries, &curves).map_err(|_| ColorError::ProfileGeneration)?;
    RgbProfile::from_generated(profile, limits)
}

fn validate_chromaticities(value: PngChrmFields) -> Result<(), ColorError> {
    let points = [
        (value.white_x, value.white_y),
        (value.red_x, value.red_y),
        (value.green_x, value.green_y),
        (value.blue_x, value.blue_y),
    ];
    for (x, y) in points {
        if x == 0 || y == 0 || u64::from(x) + u64::from(y) > 100_000 {
            return Err(ColorError::InvalidPngChromaticity);
        }
    }
    let red = (f64::from(value.red_x), f64::from(value.red_y));
    let green = (f64::from(value.green_x), f64::from(value.green_y));
    let blue = (f64::from(value.blue_x), f64::from(value.blue_y));
    let twice_area = (green.0 - red.0) * (blue.1 - red.1) - (green.1 - red.1) * (blue.0 - red.0);
    if twice_area.abs() < 1.0 {
        return Err(ColorError::InvalidPngChromaticity);
    }
    Ok(())
}

fn xyy(x: u32, y: u32) -> CIExyY {
    CIExyY {
        x: f64::from(x) / CHROMATICITY_SCALE,
        y: f64::from(y) / CHROMATICITY_SCALE,
        Y: 1.0,
    }
}

impl PngChrmFields {
    pub const SRGB: Self = Self {
        white_x: 31_270,
        white_y: 32_900,
        red_x: 64_000,
        red_y: 33_000,
        green_x: 30_000,
        green_y: 60_000,
        blue_x: 15_000,
        blue_y: 6_000,
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    fn srgb_declarations<'a>() -> PngColorDeclarations<'a> {
        PngColorDeclarations {
            srgb_rendering_intent: PngChunk::one(0),
            gamma_times_100000: PngChunk::one(1),
            chromaticities_times_100000: PngChunk::one(PngChrmFields {
                red_x: 1,
                ..PngChrmFields::SRGB
            }),
            ..PngColorDeclarations::default()
        }
    }

    #[test]
    fn priority_ignores_invalid_lower_declarations() {
        let mut declarations = srgb_declarations();
        declarations.gamma_times_100000 = PngChunk::duplicated(0);
        let resolved =
            resolve_png_color_declarations(declarations, &ResourceLimits::default()).unwrap();
        assert!(matches!(
            resolved.provenance,
            ColorProvenance::PngDeclared {
                selected: PngSelectedColorSource::Srgb,
                ..
            }
        ));
        assert_eq!(
            resolved.working_signal_relation,
            SignalRelation::LinearizedDisplayReferred
        );
    }

    #[test]
    fn cicp_has_priority_and_hdr_is_loudly_unsupported() {
        let mut declarations = srgb_declarations();
        declarations.cicp = PngChunk::one(PngCicpFields {
            color_primaries: 1,
            transfer_function: 13,
            matrix_coefficients: 0,
            video_full_range: true,
        });
        let resolved =
            resolve_png_color_declarations(declarations, &ResourceLimits::default()).unwrap();
        let ColorProvenance::PngDeclared {
            selected,
            declarations: recorded,
            ..
        } = resolved.provenance
        else {
            panic!("expected PNG provenance")
        };
        assert_eq!(selected, PngSelectedColorSource::Cicp);
        assert_eq!(recorded.cicp, declarations.cicp.value);
        declarations.cicp.value.as_mut().unwrap().transfer_function = 16;
        assert_eq!(
            resolve_png_color_declarations(declarations, &ResourceLimits::default()).unwrap_err(),
            ColorError::UnsupportedHdrPngCicp
        );
    }

    #[test]
    fn selected_duplicates_and_invalid_fields_are_rejected() {
        let declarations = PngColorDeclarations {
            srgb_rendering_intent: PngChunk::duplicated(0),
            ..PngColorDeclarations::default()
        };
        assert_eq!(
            resolve_png_color_declarations(declarations, &ResourceLimits::default()).unwrap_err(),
            ColorError::DuplicatePngDeclaration(PngColorChunk::Srgb)
        );
        let declarations = PngColorDeclarations {
            srgb_rendering_intent: PngChunk::one(4),
            ..PngColorDeclarations::default()
        };
        assert_eq!(
            resolve_png_color_declarations(declarations, &ResourceLimits::default()).unwrap_err(),
            ColorError::InvalidPngSrgbIntent
        );
    }

    #[test]
    fn bt709_cicp_is_supported_and_selected_iccp_is_bounded() {
        let declarations = PngColorDeclarations {
            cicp: PngChunk::one(PngCicpFields {
                color_primaries: 1,
                transfer_function: 1,
                matrix_coefficients: 0,
                video_full_range: true,
            }),
            ..PngColorDeclarations::default()
        };
        resolve_png_color_declarations(declarations, &ResourceLimits::default()).unwrap();

        let limits = ResourceLimits::default();
        let bytes = srgb_profile(&limits).unwrap().to_icc(&limits).unwrap();
        let mut tiny = limits;
        tiny.max_icc_bytes = 16;
        let declarations = PngColorDeclarations {
            iccp: PngChunk::one(bytes.as_slice()),
            srgb_rendering_intent: PngChunk::one(0),
            ..PngColorDeclarations::default()
        };
        assert!(matches!(
            resolve_png_color_declarations(declarations, &tiny),
            Err(ColorError::Limit(
                crate::io::LimitError::MetadataBytes { .. }
            ))
        ));
    }

    #[test]
    fn raw_gamma_and_chromaticities_are_canonical_provenance() {
        let declarations = PngColorDeclarations {
            gamma_times_100000: PngChunk::one(45_455),
            chromaticities_times_100000: PngChunk::one(PngChrmFields::SRGB),
            ..PngColorDeclarations::default()
        };
        let resolved =
            resolve_png_color_declarations(declarations, &ResourceLimits::default()).unwrap();
        let ColorProvenance::PngDeclared {
            selected,
            declarations,
            resolved_profile,
        } = resolved.provenance
        else {
            panic!("expected PNG provenance")
        };
        assert_eq!(selected, PngSelectedColorSource::ChromaticitiesAndGamma);
        assert_eq!(declarations.gamma_times_100000, Some(45_455));
        assert_eq!(
            declarations.chromaticities_times_100000,
            Some(PngChrmFields::SRGB)
        );
        assert_eq!(resolved_profile.lcms_version, lcms2::version());
        assert_ne!(resolved_profile.sha256, [0; 32]);
    }

    #[test]
    fn raw_gamma_zero_is_rejected_without_reciprocal_overflow() {
        let declarations = PngColorDeclarations {
            gamma_times_100000: PngChunk::one(0),
            chromaticities_times_100000: PngChunk::one(PngChrmFields::SRGB),
            ..PngColorDeclarations::default()
        };
        assert_eq!(
            resolve_png_color_declarations(declarations, &ResourceLimits::default()).unwrap_err(),
            ColorError::InvalidPngGamma
        );
        let declarations = PngColorDeclarations {
            gamma_times_100000: PngChunk::one(1),
            chromaticities_times_100000: PngChunk::one(PngChrmFields::SRGB),
            ..PngColorDeclarations::default()
        };
        resolve_png_color_declarations(declarations, &ResourceLimits::default()).unwrap();
    }
}
