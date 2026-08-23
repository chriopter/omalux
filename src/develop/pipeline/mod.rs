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

/// Exact peak for currently proven stage combinations. Unsupported spatial or
/// dynamically allocating stages fail before any image allocation or mutation.
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
    let peak_bytes = source_image_bytes
        .checked_add(transactional_image_bytes)
        .ok_or(PipelineError::ResourceLimit(LimitError::ArithmeticOverflow))?;
    if peak_bytes > limits.max_working_bytes {
        return Err(PipelineError::ResourceLimit(LimitError::WorkingBytes {
            requested: peak_bytes,
            maximum: limits.max_working_bytes,
        }));
    }
    Ok(DevelopWorkingSetEstimate {
        profile: DevelopWorkingSetProfile::PointwiseV1,
        pixels,
        source_image_bytes,
        transactional_image_bytes,
        stage_scratch_bytes: 0,
        peak_bytes,
    })
}

fn unproven_stage(settings: &DevelopSettings) -> Option<DevelopStage> {
    if !settings.geometry.is_neutral() {
        return Some(DevelopStage::Geometry);
    }
    if settings.basics.clarity != 0.0 {
        return Some(DevelopStage::Basics);
    }
    if !settings.tone_curves.is_neutral() {
        return Some(DevelopStage::ToneCurves);
    }
    if !settings.color_mixer.is_neutral() {
        return Some(DevelopStage::ColorMixer);
    }
    if !settings.color_grading.is_neutral() {
        return Some(DevelopStage::ColorGrading);
    }
    if !settings.radial_masks.is_neutral() {
        return Some(DevelopStage::RadialMasks);
    }
    if settings.effects.bloom != 0.0
        || settings.effects.halation != 0.0
        || settings.effects.sharpness != 0.0
    {
        return Some(DevelopStage::Effects);
    }
    None
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
