use std::fmt;

use crate::{
    develop::RgbaPixel,
    io::{LimitError, ResourceLimits, SignalRelation},
};

const REC2020_LUMA: [f64; 3] = [0.262_700_2, 0.677_998_1, 0.059_301_7];
const SCENE_MIDDLE_GREY: f64 = 0.18;
const DISPLAY_MIDDLE_GREY: f64 = 0.18;
const LOG_CONTRAST: f64 = 1.7;

/// Versioned, non-creative scene-to-display rendering method.
///
/// This is an output-boundary policy, not a preset or develop control. A new
/// rendering curve or target-gamut policy must receive a new enum variant.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum SceneRenderAlgorithm {
    LogLogisticSrgbV1,
}

/// Fixed scene-to-display renderer for an SDR sRGB destination.
#[derive(Clone, Copy, Debug, Default)]
pub struct SceneToDisplayTransform;

impl SceneToDisplayTransform {
    pub const fn new() -> Self {
        Self
    }

    /// Renders one caller-bounded scanline transactionally.
    ///
    /// The source must be scene-related linear Rec.2020. On success the
    /// destination is display-referred linear Rec.2020 suitable for a later
    /// output-profile transform. Alpha is copied bit-for-bit. Any error leaves
    /// the destination unchanged.
    pub fn transform_scanline(
        &self,
        source: &[RgbaPixel],
        destination: &mut [RgbaPixel],
        input_relation: SignalRelation,
        limits: &ResourceLimits,
    ) -> Result<SceneRenderReport, SceneRenderError> {
        if input_relation != SignalRelation::SceneRelatedRaw {
            return Err(SceneRenderError::InvalidSignalRelation {
                expected: SignalRelation::SceneRelatedRaw,
                actual: input_relation,
            });
        }
        if source.len() != destination.len() {
            return Err(SceneRenderError::LengthMismatch {
                source: source.len(),
                destination: destination.len(),
            });
        }
        estimate_scene_render_working_set(source.len(), limits)?;

        let mut scratch = Vec::new();
        scratch
            .try_reserve_exact(source.len())
            .map_err(|_| SceneRenderError::Allocation)?;
        let mut tone_mapped_pixels = 0_u64;
        let mut nonpositive_luminance_pixels = 0_u64;
        for (index, pixel) in source.iter().enumerate() {
            let (rendered, tone_mapped, nonpositive) = render_tone(pixel);
            if !rendered[..3].iter().all(|sample| sample.is_finite()) {
                return Err(SceneRenderError::NonFiniteOutput { pixel: index });
            }
            tone_mapped_pixels += u64::from(tone_mapped);
            nonpositive_luminance_pixels += u64::from(nonpositive);
            scratch.push(
                RgbaPixel::new(
                    rendered[0] as f32,
                    rendered[1] as f32,
                    rendered[2] as f32,
                    pixel.alpha(),
                )
                .map_err(|_| SceneRenderError::NonFiniteOutput { pixel: index })?,
            );
        }
        destination.copy_from_slice(&scratch);
        Ok(SceneRenderReport {
            algorithm: SceneRenderAlgorithm::LogLogisticSrgbV1,
            input_signal_relation: SignalRelation::SceneRelatedRaw,
            output_signal_relation: SignalRelation::LinearizedDisplayReferred,
            tone_mapped_pixels,
            gamut_compressed_pixels: 0,
            nonpositive_luminance_pixels,
        })
    }
}

fn render_tone(pixel: &RgbaPixel) -> ([f64; 4], bool, bool) {
    let rgb = [
        f64::from(pixel.red()),
        f64::from(pixel.green()),
        f64::from(pixel.blue()),
    ];
    let luminance = dot(rgb, REC2020_LUMA);
    if luminance <= 0.0 {
        return ([0.0, 0.0, 0.0, f64::from(pixel.alpha())], true, true);
    }
    let mapped_luminance = log_logistic_luminance(luminance);
    let scale = mapped_luminance / luminance;
    (
        [
            rgb[0] * scale,
            rgb[1] * scale,
            rgb[2] * scale,
            f64::from(pixel.alpha()),
        ],
        mapped_luminance.to_bits() != luminance.to_bits(),
        false,
    )
}

fn log_logistic_luminance(luminance: f64) -> f64 {
    debug_assert!(luminance.is_finite() && luminance > 0.0);
    let middle_logit = (DISPLAY_MIDDLE_GREY / (1.0 - DISPLAY_MIDDLE_GREY)).ln();
    let coordinate = middle_logit + LOG_CONTRAST * (luminance / SCENE_MIDDLE_GREY).ln();
    if coordinate >= 0.0 {
        1.0 / (1.0 + (-coordinate).exp())
    } else {
        let exponential = coordinate.exp();
        exponential / (1.0 + exponential)
    }
}

fn dot(left: [f64; 3], right: [f64; 3]) -> f64 {
    left[0] * right[0] + left[1] * right[1] + left[2] * right[2]
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SceneRenderWorkingSetEstimate {
    pub pixels: u64,
    pub scratch_bytes: u64,
}

/// Accounts the transactional output scratch used for one caller-bounded row.
pub fn estimate_scene_render_working_set(
    pixels: usize,
    limits: &ResourceLimits,
) -> Result<SceneRenderWorkingSetEstimate, SceneRenderError> {
    limits.validate()?;
    let pixels = u64::try_from(pixels).map_err(|_| LimitError::ArithmeticOverflow)?;
    let maximum_pixels = limits.max_pixels.min(u64::from(u32::MAX));
    if pixels > maximum_pixels {
        return Err(LimitError::PixelCount {
            requested: pixels,
            maximum: maximum_pixels,
        }
        .into());
    }
    let scratch_bytes = pixels
        .checked_mul(16)
        .ok_or(LimitError::ArithmeticOverflow)?;
    if scratch_bytes > limits.max_working_bytes {
        return Err(LimitError::WorkingBytes {
            requested: scratch_bytes,
            maximum: limits.max_working_bytes,
        }
        .into());
    }
    Ok(SceneRenderWorkingSetEstimate {
        pixels,
        scratch_bytes,
    })
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SceneRenderReport {
    pub algorithm: SceneRenderAlgorithm,
    pub input_signal_relation: SignalRelation,
    pub output_signal_relation: SignalRelation,
    pub tone_mapped_pixels: u64,
    pub gamut_compressed_pixels: u64,
    pub nonpositive_luminance_pixels: u64,
}

#[derive(Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum SceneRenderError {
    Limit(LimitError),
    InvalidSignalRelation {
        expected: SignalRelation,
        actual: SignalRelation,
    },
    LengthMismatch {
        source: usize,
        destination: usize,
    },
    Allocation,
    NonFiniteOutput {
        pixel: usize,
    },
}

impl fmt::Display for SceneRenderError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Limit(error) => error.fmt(formatter),
            Self::InvalidSignalRelation { expected, actual } => write!(
                formatter,
                "scene renderer expected {expected:?} input, received {actual:?}"
            ),
            Self::LengthMismatch {
                source,
                destination,
            } => write!(
                formatter,
                "source pixel count {source} does not match destination pixel count {destination}"
            ),
            Self::Allocation => formatter.write_str("scene renderer scratch allocation failed"),
            Self::NonFiniteOutput { pixel } => {
                write!(
                    formatter,
                    "scene renderer produced a non-finite pixel at {pixel}"
                )
            }
        }
    }
}

impl std::error::Error for SceneRenderError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Limit(error) => Some(error),
            _ => None,
        }
    }
}

impl From<LimitError> for SceneRenderError {
    fn from(error: LimitError) -> Self {
        Self::Limit(error)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn estimator_is_checked_before_allocation() {
        let limits = ResourceLimits {
            max_pixels: 3,
            ..ResourceLimits::default()
        };
        assert!(matches!(
            estimate_scene_render_working_set(4, &limits),
            Err(SceneRenderError::Limit(LimitError::PixelCount { .. }))
        ));
        assert_eq!(
            estimate_scene_render_working_set(3, &limits).unwrap(),
            SceneRenderWorkingSetEstimate {
                pixels: 3,
                scratch_bytes: 48,
            }
        );
    }

    #[test]
    fn log_logistic_curve_is_stable_and_anchored() {
        assert!((log_logistic_luminance(0.18) - 0.18).abs() < 1.0e-15);
        assert_eq!(log_logistic_luminance(f64::MIN_POSITIVE), 0.0);
        let samples = [1.0e-12, 0.001, 0.18, 1.0, 4.0, f64::MAX];
        let mapped: Vec<_> = samples.into_iter().map(log_logistic_luminance).collect();
        assert!(mapped.windows(2).all(|pair| pair[0] <= pair[1]));
        assert!(mapped.iter().all(|value| (0.0..=1.0).contains(value)));
        assert_eq!(mapped.last(), Some(&1.0));
    }

    #[test]
    fn relation_and_failures_are_transactional() {
        let source = [RgbaPixel::new(0.18, 0.18, 0.18, 0.25).unwrap()];
        let original = [RgbaPixel::new(3.0, -2.0, 1.0, 0.5).unwrap()];
        let mut destination = original;
        let renderer = SceneToDisplayTransform::new();
        assert!(matches!(
            renderer.transform_scanline(
                &source,
                &mut destination,
                SignalRelation::LinearizedDisplayReferred,
                &ResourceLimits::default(),
            ),
            Err(SceneRenderError::InvalidSignalRelation { .. })
        ));
        assert_eq!(destination, original);
    }
}
