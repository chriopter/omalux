use crate::develop::{CpuImage, DevelopStage, PipelineError, settings::GeometrySettings};

pub(super) fn apply(
    _image: &mut CpuImage,
    settings: &GeometrySettings,
) -> Result<(), PipelineError> {
    settings
        .is_neutral()
        .then_some(())
        .ok_or(PipelineError::StageNotImplemented(DevelopStage::Geometry))
}
