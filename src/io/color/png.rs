use lcms2::{CIExyY, CIExyYTRIPLE, Profile, ToneCurve};

use super::{ColorError, RgbProfile, srgb_profile};

const PNG_SRGB_GAMMA: f64 = 0.45455;
const DECLARATION_TOLERANCE: f64 = 0.00005;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Chromaticity {
    pub x: f64,
    pub y: f64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PngChromaticities {
    pub white: Chromaticity,
    pub red: Chromaticity,
    pub green: Chromaticity,
    pub blue: Chromaticity,
}

impl PngChromaticities {
    pub const SRGB: Self = Self {
        white: Chromaticity {
            x: 0.3127,
            y: 0.3290,
        },
        red: Chromaticity { x: 0.64, y: 0.33 },
        green: Chromaticity { x: 0.30, y: 0.60 },
        blue: Chromaticity { x: 0.15, y: 0.06 },
    };
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct PngColorDeclarations {
    pub srgb: bool,
    /// PNG gAMA image exponent, e.g. 0.45455 for the conventional sRGB declaration.
    pub gamma: Option<f64>,
    pub chromaticities: Option<PngChromaticities>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PngProfileKind {
    DeclaredSrgb,
    GammaChromaticities,
}

#[derive(Debug)]
pub struct SynthesizedPngProfile {
    pub profile: RgbProfile,
    pub kind: PngProfileKind,
}

/// Resolves PNG color declarations without guessing missing pairs.
///
/// An sRGB chunk may coexist only with canonical gAMA/cHRM declarations;
/// contradictory declarations are rejected instead of applying precedence.
pub fn synthesize_png_profile(
    declarations: PngColorDeclarations,
) -> Result<SynthesizedPngProfile, ColorError> {
    if declarations.srgb {
        if declarations
            .gamma
            .is_some_and(|gamma| !near(gamma, PNG_SRGB_GAMMA))
            || declarations.chromaticities.is_some_and(|chromaticities| {
                !near_chromaticities(chromaticities, PngChromaticities::SRGB)
            })
        {
            return Err(ColorError::ConflictingPngDeclaration);
        }
        return Ok(SynthesizedPngProfile {
            profile: srgb_profile(),
            kind: PngProfileKind::DeclaredSrgb,
        });
    }

    let (gamma, chromaticities) = match (declarations.gamma, declarations.chromaticities) {
        (Some(gamma), Some(chromaticities)) => (gamma, chromaticities),
        (None, None) | (Some(_), None) | (None, Some(_)) => {
            return Err(ColorError::IncompletePngDeclaration);
        }
    };
    if !gamma.is_finite() || gamma <= 0.0 || gamma > 10.0 {
        return Err(ColorError::InvalidPngGamma);
    }
    validate_chromaticities(chromaticities)?;

    let transfer = ToneCurve::new(1.0 / gamma);
    let curves = [&transfer, &transfer, &transfer];
    let white = xyy(chromaticities.white);
    let primaries = CIExyYTRIPLE {
        Red: xyy(chromaticities.red),
        Green: xyy(chromaticities.green),
        Blue: xyy(chromaticities.blue),
    };
    let profile =
        Profile::new_rgb(&white, &primaries, &curves).map_err(|_| ColorError::ProfileGeneration)?;
    Ok(SynthesizedPngProfile {
        profile: RgbProfile::from_generated(profile)?,
        kind: PngProfileKind::GammaChromaticities,
    })
}

fn validate_chromaticities(value: PngChromaticities) -> Result<(), ColorError> {
    for point in [value.white, value.red, value.green, value.blue] {
        if !point.x.is_finite()
            || !point.y.is_finite()
            || point.x <= 0.0
            || point.y <= 0.0
            || point.x + point.y > 1.0
        {
            return Err(ColorError::InvalidPngChromaticity);
        }
    }
    // Degenerate or collinear primaries cannot form an RGB color space.
    let twice_area = (value.green.x - value.red.x) * (value.blue.y - value.red.y)
        - (value.green.y - value.red.y) * (value.blue.x - value.red.x);
    if !twice_area.is_finite() || twice_area.abs() < 1.0e-8 {
        return Err(ColorError::InvalidPngChromaticity);
    }
    Ok(())
}

fn xyy(value: Chromaticity) -> CIExyY {
    CIExyY {
        x: value.x,
        y: value.y,
        Y: 1.0,
    }
}

fn near(left: f64, right: f64) -> bool {
    left.is_finite() && (left - right).abs() <= DECLARATION_TOLERANCE
}

fn near_chromaticities(left: PngChromaticities, right: PngChromaticities) -> bool {
    [left.white, left.red, left.green, left.blue]
        .into_iter()
        .zip([right.white, right.red, right.green, right.blue])
        .all(|(left, right)| near(left.x, right.x) && near(left.y, right.y))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonical_srgb_declarations_agree() {
        let resolved = synthesize_png_profile(PngColorDeclarations {
            srgb: true,
            gamma: Some(PNG_SRGB_GAMMA),
            chromaticities: Some(PngChromaticities::SRGB),
        })
        .unwrap();
        assert_eq!(resolved.kind, PngProfileKind::DeclaredSrgb);
    }

    #[test]
    fn conflicting_or_incomplete_declarations_are_rejected() {
        assert_eq!(
            synthesize_png_profile(PngColorDeclarations {
                srgb: true,
                gamma: Some(1.0),
                chromaticities: None,
            })
            .unwrap_err(),
            ColorError::ConflictingPngDeclaration
        );
        assert_eq!(
            synthesize_png_profile(PngColorDeclarations {
                gamma: Some(PNG_SRGB_GAMMA),
                ..PngColorDeclarations::default()
            })
            .unwrap_err(),
            ColorError::IncompletePngDeclaration
        );
    }

    #[test]
    fn gamma_and_chromaticities_are_validated_before_lcms() {
        for gamma in [0.0, -1.0, f64::NAN, f64::INFINITY, 10.1] {
            assert_eq!(
                synthesize_png_profile(PngColorDeclarations {
                    gamma: Some(gamma),
                    chromaticities: Some(PngChromaticities::SRGB),
                    srgb: false,
                })
                .unwrap_err(),
                ColorError::InvalidPngGamma
            );
        }
        let mut invalid = PngChromaticities::SRGB;
        invalid.red.x = f64::NAN;
        assert_eq!(
            synthesize_png_profile(PngColorDeclarations {
                gamma: Some(PNG_SRGB_GAMMA),
                chromaticities: Some(invalid),
                srgb: false,
            })
            .unwrap_err(),
            ColorError::InvalidPngChromaticity
        );
    }

    #[test]
    fn valid_gamma_chromaticity_pair_creates_matrix_profile() {
        let resolved = synthesize_png_profile(PngColorDeclarations {
            gamma: Some(0.5),
            chromaticities: Some(PngChromaticities::SRGB),
            srgb: false,
        })
        .unwrap();
        assert_eq!(resolved.kind, PngProfileKind::GammaChromaticities);
        assert!(resolved.profile.is_matrix_shaper());
    }
}
