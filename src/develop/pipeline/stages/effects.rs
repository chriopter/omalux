use crate::develop::{CpuImage, DevelopStage, PipelineError, settings::EffectsSettings};

mod optical;
mod spatial;

pub(super) fn supports(settings: &EffectsSettings) -> bool {
    settings.fade == 0.0
        && settings.vignette == 0.0
        && settings.sharpness == 0.0
        && settings.grain.amount == 0.0
}

pub(super) fn apply(image: &mut CpuImage, settings: &EffectsSettings) -> Result<(), PipelineError> {
    if !supports(settings) {
        return Err(PipelineError::StageNotImplemented(DevelopStage::Effects));
    }
    optical::apply_bloom(image, settings.bloom);
    optical::apply_halation(image, settings.halation);
    Ok(())
}
