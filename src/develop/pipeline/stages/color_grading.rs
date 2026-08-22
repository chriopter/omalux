use crate::develop::{CpuImage, DevelopStage, PipelineError, settings::ColorGradingSettings};

pub(super) fn apply(
    _image: &mut CpuImage,
    settings: &ColorGradingSettings,
) -> Result<(), PipelineError> {
    settings
        .is_neutral()
        .then_some(())
        .ok_or(PipelineError::StageNotImplemented(
            DevelopStage::ColorGrading,
        ))
}
