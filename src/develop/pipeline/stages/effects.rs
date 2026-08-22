use crate::develop::{CpuImage, DevelopStage, PipelineError, settings::EffectsSettings};

mod optical;
mod spatial;
mod tonal;

pub(super) fn supports(settings: &EffectsSettings) -> bool {
    settings.grain.amount == 0.0
}

pub(super) fn apply(image: &mut CpuImage, settings: &EffectsSettings) -> Result<(), PipelineError> {
    if !supports(settings) {
        return Err(PipelineError::StageNotImplemented(DevelopStage::Effects));
    }
    // Persisted effect order. Keep this stable because the operations do not
    // generally commute and preset rendering depends on it.
    optical::apply_bloom(image, settings.bloom);
    optical::apply_halation(image, settings.halation);
    tonal::apply_fade(image, settings.fade);
    tonal::apply_vignette(image, settings.vignette);
    tonal::apply_sharpness(image, settings.sharpness);
    Ok(())
}
