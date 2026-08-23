use std::fmt;

use crate::io::{LimitError, ResourceLimits, SignalRelation};

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
}
