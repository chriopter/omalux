use crate::develop::{CpuImage, DevelopStage, PipelineError, settings::ColorGradingSettings};

pub(super) fn supports(settings: &ColorGradingSettings) -> bool {
    settings.is_neutral()
}

pub(super) fn apply(
    _image: &mut CpuImage,
    settings: &ColorGradingSettings,
) -> Result<(), PipelineError> {
    supports(settings)
        .then_some(())
        .ok_or(PipelineError::StageNotImplemented(
            DevelopStage::ColorGrading,
        ))
}
