use crate::develop::{CpuImage, DevelopStage, PipelineError, settings::BasicsSettings};

pub(super) fn supports(settings: &BasicsSettings) -> bool {
    settings.is_neutral()
}

pub(super) fn apply(_image: &mut CpuImage, settings: &BasicsSettings) -> Result<(), PipelineError> {
    supports(settings)
        .then_some(())
        .ok_or(PipelineError::StageNotImplemented(DevelopStage::Basics))
}
