mod stages;

use super::{
    CANONICAL_STAGE_ORDER, CpuImage, DevelopSettings, DevelopStage, ImageError, SettingsError,
};
use std::fmt;

#[derive(Clone, Debug, Default)]
pub struct DevelopPipeline;

impl DevelopPipeline {
    pub const fn stages(&self) -> &'static [DevelopStage] {
        &CANONICAL_STAGE_ORDER
    }

    /// Validates the document and verifies that every non-neutral stage is
    /// supported before rendering begins.
    pub fn preflight(&self, settings: &DevelopSettings) -> Result<(), PipelineError> {
        settings
            .validate()
            .map_err(PipelineError::InvalidSettings)?;
        for stage in CANONICAL_STAGE_ORDER {
            stages::ensure_supported(stage, settings)?;
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
        image.validate().map_err(PipelineError::InvalidImage)?;
        self.preflight(settings)?;

        let mut rendered = image.clone();
        for stage in CANONICAL_STAGE_ORDER {
            stages::apply(stage, &mut rendered, settings)?;
        }
        rendered.validate().map_err(PipelineError::InvalidImage)?;
        *image = rendered;
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum PipelineError {
    InvalidImage(ImageError),
    InvalidSettings(SettingsError),
    StageNotImplemented(DevelopStage),
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
            Self::StageNotImplemented(_) | Self::NumericFailure { .. } => None,
        }
    }
}
