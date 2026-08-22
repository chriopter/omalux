use crate::develop::{CpuImage, DevelopStage, PipelineError, settings::RadialMasksSettings};

pub(super) fn supports(settings: &RadialMasksSettings) -> bool {
    settings.is_neutral()
}

pub(super) fn apply(
    _image: &mut CpuImage,
    settings: &RadialMasksSettings,
) -> Result<(), PipelineError> {
    supports(settings)
        .then_some(())
        .ok_or(PipelineError::StageNotImplemented(
            DevelopStage::RadialMasks,
        ))
}
