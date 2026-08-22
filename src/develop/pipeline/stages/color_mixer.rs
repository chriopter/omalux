use crate::develop::{CpuImage, DevelopStage, PipelineError, settings::ColorMixerSettings};

#[path = "../../color.rs"]
mod color;

pub(super) fn supports(settings: &ColorMixerSettings) -> bool {
    settings.is_neutral()
}

pub(super) fn apply(
    _image: &mut CpuImage,
    settings: &ColorMixerSettings,
) -> Result<(), PipelineError> {
    supports(settings)
        .then_some(())
        .ok_or(PipelineError::StageNotImplemented(DevelopStage::ColorMixer))
}
