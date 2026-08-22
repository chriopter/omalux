use crate::develop::{CpuImage, DevelopStage, PipelineError, settings::ToneCurvesSettings};

pub(super) fn apply(
    _image: &mut CpuImage,
    settings: &ToneCurvesSettings,
) -> Result<(), PipelineError> {
    settings
        .is_neutral()
        .then_some(())
        .ok_or(PipelineError::StageNotImplemented(DevelopStage::ToneCurves))
}
