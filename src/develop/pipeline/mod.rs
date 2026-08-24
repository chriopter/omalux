mod stages;

use super::{
    CANONICAL_STAGE_ORDER, CpuImage, DevelopRenderContext, DevelopSettings, DevelopStage,
    ImageError, SettingsError,
};
use crate::io::{LimitError, ResourceLimits};
use std::fmt;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct DevelopWorkingSetProfile {
    /// Reviewed bounded prepared tone curves and allocation-free color state.
    pub color_v1: bool,
    /// Reviewed full-frame clarity/effects family.
    pub spatial_v1: bool,
    /// Reviewed orthogonal/projective/crop geometry family.
    pub geometry_v1: bool,
    /// Reviewed analytic radial-mask/local-adjustment family.
    pub radial_masks_v1: bool,
}

#[allow(non_upper_case_globals)]
impl DevelopWorkingSetProfile {
    /// Compatibility names for the four profiles admitted before family
    /// composition became explicit. PointwiseV1 is the all-false base.
    pub const PointwiseV1: Self = Self::new(false, false, false, false);
    pub const ColorV1: Self = Self::new(true, false, false, false);
    pub const SpatialV1: Self = Self::new(false, true, false, false);
    pub const ColorSpatialV1: Self = Self::new(true, true, false, false);

    pub const fn new(
        color_v1: bool,
        spatial_v1: bool,
        geometry_v1: bool,
        radial_masks_v1: bool,
    ) -> Self {
        Self {
            color_v1,
            spatial_v1,
            geometry_v1,
            radial_masks_v1,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DevelopWorkingSetEstimate {
    pub profile: DevelopWorkingSetProfile,
    pub pixels: u64,
    pub source_image_bytes: u64,
    pub transactional_image_bytes: u64,
    pub output_width: u32,
    pub output_height: u32,
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
/// Applies only the pointwise color stages: basics with clarity forced to
/// zero, tone curves, color mixer, and color grading. Geometry, clarity,
/// radial masks, and effects are deliberately excluded — every remaining
/// operation maps each pixel independently of its neighbors and position, so
/// the composition can be sampled into a lookup table for interactive
/// previews. This is a preview aid, not the normative render: callers must
/// still produce final output through the full pipeline.
pub fn apply_point_color_operations(
    image: &mut CpuImage,
    settings: &DevelopSettings,
) -> Result<(), PipelineError> {
    let mut point_settings = settings.clone();
    point_settings.basics.clarity = 0.0;
    for stage in [
        DevelopStage::Basics,
        DevelopStage::ToneCurves,
        DevelopStage::ColorMixer,
        DevelopStage::ColorGrading,
    ] {
        stages::apply(stage, image, &point_settings, None)?;
    }
    Ok(())
}

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
    if radial_masks_have_negative_sharpness(settings) {
        return Err(PipelineError::ResourceProfileUnavailable(
            DevelopStage::RadialMasks,
        ));
    }
    let source_image_bytes = pixels
        .checked_mul(16)
        .ok_or(PipelineError::ResourceLimit(LimitError::ArithmeticOverflow))?;
    let transactional_image_bytes = source_image_bytes;
    let geometry_active = !settings.geometry.is_neutral();
    let (develop_width, develop_height, geometry_scratch_bytes) = if geometry_active {
        stages::geometry_v1_working_set(width, height, &settings.geometry)?
    } else {
        (width, height, 0)
    };
    let develop_pixels = u64::from(develop_width)
        .checked_mul(u64::from(develop_height))
        .ok_or(PipelineError::ResourceLimit(LimitError::ArithmeticOverflow))?;
    let developed_image_bytes = develop_pixels
        .checked_mul(16)
        .ok_or(PipelineError::ResourceLimit(LimitError::ArithmeticOverflow))?;
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
        spatial_stage_scratch_bytes(develop_width, develop_height, develop_pixels, settings)?
    } else {
        0
    };
    let radial_masks_active = !settings.radial_masks.is_neutral();
    let radial_masks_scratch_bytes = if radial_masks_active {
        stages::radial_masks_v1_scratch_bytes(
            develop_width,
            develop_height,
            &settings.radial_masks,
        )?
    } else {
        0
    };

    // Geometry runs while the original transaction image is still resident.
    // Once it commits into that transaction, later families run sequentially
    // against the possibly cropped dimensions.
    let clone_peak = source_image_bytes
        .checked_add(transactional_image_bytes)
        .ok_or(PipelineError::ResourceLimit(LimitError::ArithmeticOverflow))?;
    let geometry_peak = clone_peak
        .checked_add(geometry_scratch_bytes)
        .ok_or(PipelineError::ResourceLimit(LimitError::ArithmeticOverflow))?;
    let post_geometry_base = source_image_bytes
        .checked_add(developed_image_bytes)
        .ok_or(PipelineError::ResourceLimit(LimitError::ArithmeticOverflow))?;
    let later_scratch = color_scratch_bytes
        .max(spatial_scratch_bytes)
        .max(radial_masks_scratch_bytes);
    let later_peak = post_geometry_base
        .checked_add(later_scratch)
        .ok_or(PipelineError::ResourceLimit(LimitError::ArithmeticOverflow))?;
    let peak_bytes = clone_peak.max(geometry_peak).max(later_peak);
    let stage_scratch_bytes = peak_bytes - clone_peak;
    if peak_bytes > limits.max_working_bytes {
        return Err(PipelineError::ResourceLimit(LimitError::WorkingBytes {
            requested: peak_bytes,
            maximum: limits.max_working_bytes,
        }));
    }
    Ok(DevelopWorkingSetEstimate {
        profile: DevelopWorkingSetProfile::new(
            color_active,
            spatial_active,
            geometry_active,
            radial_masks_active,
        ),
        pixels,
        source_image_bytes,
        transactional_image_bytes,
        output_width: develop_width,
        output_height: develop_height,
        stage_scratch_bytes,
        peak_bytes,
    })
}

fn radial_masks_have_negative_sharpness(settings: &DevelopSettings) -> bool {
    settings
        .radial_masks
        .masks
        .iter()
        .any(|mask| mask.enabled && mask.opacity > 0.0 && mask.adjustments.sharpness < 0.0)
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
