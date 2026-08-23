mod stages;

use super::{
    CANONICAL_STAGE_ORDER, CpuImage, DevelopRenderContext, DevelopSettings, DevelopStage,
    ImageError, SettingsError,
};
use crate::io::{LimitError, ResourceLimits};
use std::fmt;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DevelopWorkingSetProfile {
    /// Transactional image copy plus allocation-free pointwise stages.
    PointwiseV1,
    /// PointwiseV1 plus bounded prepared tone curves and allocation-free
    /// color mixer/grading state.
    ColorV1,
    /// Pointwise stages plus the reviewed full-frame clarity/effects family.
    SpatialV1,
    /// The union of the reviewed ColorV1 and SpatialV1 stage families.
    ColorSpatialV1,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DevelopWorkingSetEstimate {
    pub profile: DevelopWorkingSetProfile,
    pub pixels: u64,
    pub source_image_bytes: u64,
    pub transactional_image_bytes: u64,
    pub stage_scratch_bytes: u64,
    pub peak_bytes: u64,
}

#[derive(Clone, Debug, Default)]
pub struct DevelopPipeline;

impl DevelopPipeline {
    /// Applies a settings-only bounded render without external render context.
    pub fn process_bounded(
        &self,
        image: &mut CpuImage,
        settings: &DevelopSettings,
        limits: &ResourceLimits,
    ) -> Result<DevelopWorkingSetEstimate, PipelineError> {
        self.process_bounded_with_context(image, settings, None, limits)
    }

    pub const fn stages(&self) -> &'static [DevelopStage] {
        &CANONICAL_STAGE_ORDER
    }

    /// Validates the document and verifies that every non-neutral stage is
    /// supported before rendering begins.
    pub fn preflight(&self, settings: &DevelopSettings) -> Result<(), PipelineError> {
        self.preflight_with_context(settings, None)
    }

    /// Validates settings, stage capabilities, and required non-persisted
    /// render inputs before any pixel processing begins.
    pub fn preflight_with_context(
        &self,
        settings: &DevelopSettings,
        context: Option<&DevelopRenderContext>,
    ) -> Result<(), PipelineError> {
        settings
            .validate()
            .map_err(PipelineError::InvalidSettings)?;
        for stage in CANONICAL_STAGE_ORDER {
            stages::ensure_supported(stage, settings, context)?;
        }
        Ok(())
    }

    /// Applies a settings document transactionally.
    ///
    /// Every error leaves `image` byte-for-byte unchanged. Input and output
    /// buffers are defensively checked for the normative pixel contract.
    pub fn process(
        &self,
        image: &mut CpuImage,
        settings: &DevelopSettings,
    ) -> Result<(), PipelineError> {
        self.process_with_context(image, settings, None)
    }

    /// Applies a settings document transactionally using explicit render
    /// inputs such as the source-content-derived grain seed.
    pub fn process_with_context(
        &self,
        image: &mut CpuImage,
        settings: &DevelopSettings,
        context: Option<&DevelopRenderContext>,
    ) -> Result<(), PipelineError> {
        image.validate().map_err(PipelineError::InvalidImage)?;
        self.preflight_with_context(settings, context)?;

        let mut rendered = try_clone_image(image)?;
        for stage in CANONICAL_STAGE_ORDER {
            stages::apply(stage, &mut rendered, settings, context)?;
        }
        rendered.validate().map_err(PipelineError::InvalidImage)?;
        *image = rendered;
        Ok(())
    }

    /// Preflights the named bounded-memory renderer and applies it atomically.
    pub fn process_bounded_with_context(
        &self,
        image: &mut CpuImage,
        settings: &DevelopSettings,
        context: Option<&DevelopRenderContext>,
        limits: &ResourceLimits,
    ) -> Result<DevelopWorkingSetEstimate, PipelineError> {
        image.validate().map_err(PipelineError::InvalidImage)?;
        self.preflight_with_context(settings, context)?;
        let estimate =
            estimate_validated_working_set(image.width(), image.height(), settings, limits)?;
        let mut rendered = try_clone_image(image)?;
        for stage in CANONICAL_STAGE_ORDER {
            stages::apply(stage, &mut rendered, settings, context)?;
        }
        rendered.validate().map_err(PipelineError::InvalidImage)?;
        *image = rendered;
        Ok(estimate)
    }
}

/// Reviewed requested-payload upper bound for proven stage combinations.
/// Unsupported allocation families fail before any image allocation or mutation.
pub fn estimate_develop_working_set(
    width: u32,
    height: u32,
    settings: &DevelopSettings,
    limits: &ResourceLimits,
) -> Result<DevelopWorkingSetEstimate, PipelineError> {
    settings
        .validate()
        .map_err(PipelineError::InvalidSettings)?;
    estimate_validated_working_set(width, height, settings, limits)
}

fn estimate_validated_working_set(
    width: u32,
    height: u32,
    settings: &DevelopSettings,
    limits: &ResourceLimits,
) -> Result<DevelopWorkingSetEstimate, PipelineError> {
    limits.validate().map_err(PipelineError::ResourceLimit)?;
    if width == 0 || height == 0 {
        return Err(PipelineError::ResourceLimit(LimitError::EmptyDimensions));
    }
    let pixels = u64::from(width)
        .checked_mul(u64::from(height))
        .ok_or(PipelineError::ResourceLimit(LimitError::ArithmeticOverflow))?;
    if pixels > limits.max_pixels {
        return Err(PipelineError::ResourceLimit(LimitError::PixelCount {
            requested: pixels,
            maximum: limits.max_pixels,
        }));
    }
    let unsupported = unproven_stage(settings);
    if let Some(stage) = unsupported {
        return Err(PipelineError::ResourceProfileUnavailable(stage));
    }
    let source_image_bytes = pixels
        .checked_mul(16)
        .ok_or(PipelineError::ResourceLimit(LimitError::ArithmeticOverflow))?;
    let transactional_image_bytes = source_image_bytes;
    let color_active = !settings.tone_curves.is_neutral()
        || !settings.color_mixer.is_neutral()
        || !settings.color_grading.is_neutral();
    let color_scratch_bytes = if color_active {
        stages::color_v1_heap_bytes(settings)?
    } else {
        0
    };
    let spatial_active = settings.basics.clarity != 0.0
        || settings.effects.bloom != 0.0
        || settings.effects.halation != 0.0
        || settings.effects.sharpness != 0.0;
    let spatial_scratch_bytes = if spatial_active {
        spatial_stage_scratch_bytes(width, height, pixels, settings)?
    } else {
        0
    };
    // Color and spatial stages execute sequentially, so their scratch peaks
    // are alternatives rather than simultaneously resident payloads.
    let stage_scratch_bytes = color_scratch_bytes.max(spatial_scratch_bytes);
    let peak_bytes = source_image_bytes
        .checked_add(transactional_image_bytes)
        .and_then(|bytes| bytes.checked_add(stage_scratch_bytes))
        .ok_or(PipelineError::ResourceLimit(LimitError::ArithmeticOverflow))?;
    if peak_bytes > limits.max_working_bytes {
        return Err(PipelineError::ResourceLimit(LimitError::WorkingBytes {
            requested: peak_bytes,
            maximum: limits.max_working_bytes,
        }));
    }
    Ok(DevelopWorkingSetEstimate {
        profile: if color_active && spatial_active {
            DevelopWorkingSetProfile::ColorSpatialV1
        } else if color_active {
            DevelopWorkingSetProfile::ColorV1
        } else if spatial_active {
            DevelopWorkingSetProfile::SpatialV1
        } else {
            DevelopWorkingSetProfile::PointwiseV1
        },
        pixels,
        source_image_bytes,
        transactional_image_bytes,
        stage_scratch_bytes,
        peak_bytes,
    })
}

fn unproven_stage(settings: &DevelopSettings) -> Option<DevelopStage> {
    if !settings.geometry.is_neutral() {
        return Some(DevelopStage::Geometry);
    }
    if !settings.radial_masks.is_neutral() {
        return Some(DevelopStage::RadialMasks);
    }
    None
}

fn spatial_stage_scratch_bytes(
    width: u32,
    height: u32,
    pixels: u64,
    settings: &DevelopSettings,
) -> Result<u64, PipelineError> {
    // All four covered algorithms retain at most three full f32 scalar
    // planes. Covered stages execute sequentially, so their peaks do not add.
    let planes = pixels
        .checked_mul(12)
        .ok_or(PipelineError::ResourceLimit(LimitError::ArithmeticOverflow))?;
    let tile_width = u64::from(width.min(128));
    let tile_height = u64::from(height.min(64));

    let clarity_aux = if settings.basics.clarity != 0.0 {
        tile_width
            .checked_mul(
                tile_height
                    .checked_add(16)
                    .ok_or(PipelineError::ResourceLimit(LimitError::ArithmeticOverflow))?,
            )
            .and_then(|value| value.checked_mul(16))
            .ok_or(PipelineError::ResourceLimit(LimitError::ArithmeticOverflow))?
    } else {
        0
    };
    let effects_active = settings.effects.bloom != 0.0
        || settings.effects.halation != 0.0
        || settings.effects.sharpness != 0.0;
    let effects_aux = if effects_active {
        // Gaussian residual sigma is at most 2.5, hence radius <= 8 and a
        // 17-value kernel. The 512-byte term conservatively covers all
        // pyramid dimension entries (at most 32 pairs of usize on u64).
        tile_width
            .checked_mul(
                tile_height
                    .checked_add(16)
                    .ok_or(PipelineError::ResourceLimit(LimitError::ArithmeticOverflow))?,
            )
            .and_then(|value| value.checked_mul(4))
            .and_then(|value| value.checked_add(17 * 8 + 32 * 16))
            .ok_or(PipelineError::ResourceLimit(LimitError::ArithmeticOverflow))?
    } else {
        0
    };
    planes
        .checked_add(clarity_aux.max(effects_aux))
        .ok_or(PipelineError::ResourceLimit(LimitError::ArithmeticOverflow))
}

fn try_clone_image(image: &CpuImage) -> Result<CpuImage, PipelineError> {
    let mut pixels = Vec::new();
    pixels
        .try_reserve_exact(image.pixels().len())
        .map_err(|_| PipelineError::ResourceLimit(LimitError::Allocation))?;
    pixels.extend_from_slice(image.pixels());
    CpuImage::new(image.width(), image.height(), pixels).map_err(PipelineError::InvalidImage)
}

#[derive(Clone, Debug, PartialEq)]
pub enum PipelineError {
    InvalidImage(ImageError),
    InvalidSettings(SettingsError),
    StageNotImplemented(DevelopStage),
    MissingRenderContext(DevelopStage),
    ResourceProfileUnavailable(DevelopStage),
    ResourceLimit(LimitError),
    NumericFailure {
        stage: DevelopStage,
        reason: &'static str,
    },
}

impl fmt::Display for PipelineError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidImage(error) => write!(formatter, "invalid CPU image: {error}"),
            Self::InvalidSettings(error) => write!(formatter, "invalid develop settings: {error}"),
            Self::StageNotImplemented(stage) => {
                write!(formatter, "non-neutral {stage:?} stage is not implemented")
            }
            Self::MissingRenderContext(stage) => {
                write!(
                    formatter,
                    "non-neutral {stage:?} stage requires a render context"
                )
            }
            Self::ResourceProfileUnavailable(stage) => write!(
                formatter,
                "{stage:?} has no reviewed bounded-memory develop profile"
            ),
            Self::ResourceLimit(error) => error.fmt(formatter),
            Self::NumericFailure { stage, reason } => {
                write!(formatter, "numerical failure in {stage:?}: {reason}")
            }
        }
    }
}

impl std::error::Error for PipelineError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::InvalidImage(error) => Some(error),
            Self::InvalidSettings(error) => Some(error),
            Self::ResourceLimit(error) => Some(error),
            Self::StageNotImplemented(_)
            | Self::MissingRenderContext(_)
            | Self::ResourceProfileUnavailable(_)
            | Self::NumericFailure { .. } => None,
        }
    }
}

#[cfg(test)]
mod resource_tests {
    use super::*;

    #[test]
    fn checked_image_bytes_reject_dimensions_whose_rgba_peak_overflows() {
        let limits = ResourceLimits {
            max_pixels: u64::MAX,
            max_working_bytes: u64::MAX,
            ..ResourceLimits::default()
        };
        assert_eq!(
            estimate_develop_working_set(u32::MAX, u32::MAX, &DevelopSettings::default(), &limits),
            Err(PipelineError::ResourceLimit(LimitError::ArithmeticOverflow))
        );
    }
}
