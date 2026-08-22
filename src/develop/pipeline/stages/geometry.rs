use crate::develop::{CpuImage, DevelopStage, PipelineError, settings::GeometrySettings};

pub(super) fn supports(settings: &GeometrySettings) -> bool {
    settings.is_neutral()
}

pub(super) fn apply(
    _image: &mut CpuImage,
    settings: &GeometrySettings,
) -> Result<(), PipelineError> {
    supports(settings)
        .then_some(())
        .ok_or(PipelineError::StageNotImplemented(DevelopStage::Geometry))
}
