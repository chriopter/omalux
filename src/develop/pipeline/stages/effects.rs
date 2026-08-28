use super::spatial;
use crate::develop::{
    CpuImage, DevelopRenderContext, DevelopStage, PipelineError, settings::EffectsSettings,
};

mod grain;
mod optical;
mod tonal;

pub(super) fn supports(_settings: &EffectsSettings) -> bool {
    true
}

pub(super) fn ensure_context(
    settings: &EffectsSettings,
    context: Option<&DevelopRenderContext>,
) -> Result<(), PipelineError> {
    if settings.grain.amount > 0.0 && context.is_none() {
        return Err(PipelineError::MissingRenderContext(DevelopStage::Effects));
    }
    Ok(())
}

pub(super) fn apply(
    image: &mut CpuImage,
    settings: &EffectsSettings,
    context: Option<&DevelopRenderContext>,
) -> Result<(), PipelineError> {
    ensure_context(settings, context)?;
    // Persisted effect order. Keep this stable because the operations do not
    // generally commute and preset rendering depends on it.
    if settings.bloom != 0.0 {
        optical::apply_bloom(image, settings.bloom)?;
    }
    if settings.halation != 0.0 {
        optical::apply_halation(image, settings.halation)?;
    }
    if settings.vignette != 0.0 {
        tonal::apply_vignette(image, settings.vignette);
    }
    if settings.sharpness != 0.0 {
        tonal::apply_sharpness(image, settings.sharpness)?;
    }
    if settings.grain.amount != 0.0 {
        let seed = context
            .expect("active grain context was checked before rendering")
            .grain_seed();
        grain::apply_full_image(image, &settings.grain, seed).map_err(map_grain_error)?;
    }
    // Fade is the print's density floor and ceiling, so it bounds the
    // finished image: applying it before vignette, sharpening, or grain would
    // let those operations push samples back past the limits it establishes.
    if settings.fade != 0.0 {
        tonal::apply_fade(image, settings.fade);
    }
    Ok(())
}

fn map_grain_error(error: grain::GrainError) -> PipelineError {
    let reason = match error {
        grain::GrainError::EmptyExtent => "grain extent must be non-empty",
        grain::GrainError::DimensionTooLarge => "grain extent exceeds the supported dimension",
        grain::GrainError::DimensionOverflow => "grain extent arithmetic overflowed",
        grain::GrainError::RegionOutOfBounds => "grain region is outside the full image",
        grain::GrainError::BufferLengthMismatch { .. } => {
            "grain region does not match its pixel buffer"
        }
        grain::GrainError::NonFiniteOutput { .. } => "grain produced a non-finite RGB value",
    };
    PipelineError::NumericFailure {
        stage: DevelopStage::Effects,
        reason,
    }
}

pub(super) fn local_sharpness_delta<F>(
    extent: [usize; 2],
    point: [usize; 2],
    amount: f32,
    kernel: &[f64],
    scratch: &mut [f32],
    sample_luminance: F,
) -> f64
where
    F: FnMut(usize, usize) -> f32,
{
    tonal::sharpness_delta_at(extent, point, amount, kernel, scratch, sample_luminance)
}

pub(super) fn local_sharpness_kernel() -> [f64; 7] {
    let mut kernel = [0.0; 7];
    for (index, weight) in kernel.iter_mut().enumerate() {
        let distance = index as f64 - 3.0;
        *weight = (-distance * distance / 2.0).exp();
    }
    let sum: f64 = kernel.iter().sum();
    for weight in &mut kernel {
        *weight /= sum;
    }
    kernel
}

pub(super) fn add_finite_delta(channel: f32, delta: f64) -> f32 {
    spatial::finite_f32(f64::from(channel) + delta)
}
