mod stages;

use super::{CANONICAL_STAGE_ORDER, CpuImage, DevelopSettings, DevelopStage, SettingsError};
use std::fmt;

#[derive(Clone, Debug, Default)]
pub struct DevelopPipeline;

impl DevelopPipeline {
    pub const fn stages(&self) -> &'static [DevelopStage] {
        &CANONICAL_STAGE_ORDER
    }

    pub fn process(
        &self,
        image: &mut CpuImage,
        settings: &DevelopSettings,
    ) -> Result<(), PipelineError> {
        settings
            .validate()
            .map_err(PipelineError::InvalidSettings)?;
        for stage in CANONICAL_STAGE_ORDER {
            stages::apply(stage, image, settings)?;
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum PipelineError {
    InvalidSettings(SettingsError),
    StageNotImplemented(DevelopStage),
}

impl fmt::Display for PipelineError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidSettings(error) => write!(formatter, "invalid develop settings: {error}"),
            Self::StageNotImplemented(stage) => {
                write!(formatter, "non-neutral {stage:?} stage is not implemented")
            }
        }
    }
}

impl std::error::Error for PipelineError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::InvalidSettings(error) => Some(error),
            Self::StageNotImplemented(_) => None,
        }
    }
}
