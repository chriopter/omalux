use crate::develop::{CpuImage, DevelopStage, PipelineError, settings::EffectsSettings};

mod optical;
mod spatial;
mod tonal;
// The grain kernel is ready, but Foundation has no render context from which a
// resolved image-stable seed can be obtained. Keep it compiled and tested while
// the Effects stage continues to reject non-zero grain rather than inventing a
// path- or filename-based seed here.
#[allow(dead_code)]
mod grain;

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

pub(super) fn local_sharpness_delta<F>(
    extent: [usize; 2],
    point: [usize; 2],
    amount: f32,
    kernel: &[f64],
    scratch: &mut Vec<f32>,
    sample_luminance: F,
) -> f64
where
    F: FnMut(usize, usize) -> f32,
{
    tonal::sharpness_delta_at(extent, point, amount, kernel, scratch, sample_luminance)
}

pub(super) fn local_sharpness_kernel() -> Vec<f64> {
    spatial::gaussian_kernel(1.0)
}

pub(super) fn add_finite_delta(channel: f32, delta: f64) -> f32 {
    spatial::finite_f32(f64::from(channel) + delta)
}
