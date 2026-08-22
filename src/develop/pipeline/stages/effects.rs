use crate::develop::{CpuImage, DevelopStage, PipelineError, settings::EffectsSettings};

pub(super) fn supports(settings: &EffectsSettings) -> bool {
    settings.is_neutral()
}

pub(super) fn apply(
    _image: &mut CpuImage,
    settings: &EffectsSettings,
) -> Result<(), PipelineError> {
    supports(settings)
        .then_some(())
        .ok_or(PipelineError::StageNotImplemented(DevelopStage::Effects))
}
