use crate::develop::{
    CpuImage, DevelopRenderContext, DevelopStage, PipelineError, settings::EffectsSettings,
};

mod grain;
mod optical;
mod spatial;
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
        optical::apply_bloom(image, settings.bloom);
    }
    if settings.halation != 0.0 {
        optical::apply_halation(image, settings.halation);
    }
    if settings.fade != 0.0 {
        tonal::apply_fade(image, settings.fade);
    }
    if settings.vignette != 0.0 {
        tonal::apply_vignette(image, settings.vignette);
    }
    if settings.sharpness != 0.0 {
        tonal::apply_sharpness(image, settings.sharpness);
    }
    if settings.grain.amount != 0.0 {
        let seed = context
            .expect("active grain context was checked before rendering")
            .grain_seed();
        grain::apply_full_image(image, &settings.grain, seed).map_err(map_grain_error)?;
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
