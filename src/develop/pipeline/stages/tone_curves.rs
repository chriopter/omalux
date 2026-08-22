use crate::develop::{CpuImage, DevelopStage, PipelineError, settings::ToneCurvesSettings};

pub(super) fn supports(settings: &ToneCurvesSettings) -> bool {
    settings.is_neutral()
}

pub(super) fn apply(
    _image: &mut CpuImage,
    settings: &ToneCurvesSettings,
) -> Result<(), PipelineError> {
    supports(settings)
        .then_some(())
        .ok_or(PipelineError::StageNotImplemented(DevelopStage::ToneCurves))
}
