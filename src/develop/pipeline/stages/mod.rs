mod basics;
mod color_grading;
mod color_mixer;
mod effects;
mod geometry;
mod radial_masks;
mod spatial;
mod tone_curves;

use super::{CpuImage, DevelopRenderContext, DevelopSettings, DevelopStage, PipelineError};

pub(super) fn ensure_supported(
    stage: DevelopStage,
    settings: &DevelopSettings,
    context: Option<&DevelopRenderContext>,
) -> Result<(), PipelineError> {
    let supported = match stage {
        DevelopStage::Geometry => geometry::supports(&settings.geometry),
        DevelopStage::Basics => basics::supports(&settings.basics),
        DevelopStage::ToneCurves => tone_curves::supports(&settings.tone_curves),
        DevelopStage::ColorMixer => color_mixer::supports(&settings.color_mixer),
        DevelopStage::ColorGrading => color_grading::supports(&settings.color_grading),
        DevelopStage::RadialMasks => radial_masks::supports(&settings.radial_masks),
        DevelopStage::Effects => effects::supports(&settings.effects),
    };
    supported
        .then_some(())
        .ok_or(PipelineError::StageNotImplemented(stage))?;
    if stage == DevelopStage::Effects {
        effects::ensure_context(&settings.effects, context)?;
    }
    Ok(())
}

pub(super) fn apply(
    stage: DevelopStage,
    image: &mut CpuImage,
    settings: &DevelopSettings,
    context: Option<&DevelopRenderContext>,
) -> Result<(), PipelineError> {
    match stage {
        DevelopStage::Geometry => geometry::apply(image, &settings.geometry),
        DevelopStage::Basics => basics::apply(image, &settings.basics),
        DevelopStage::ToneCurves => tone_curves::apply(image, &settings.tone_curves),
        DevelopStage::ColorMixer => color_mixer::apply(image, &settings.color_mixer),
        DevelopStage::ColorGrading => color_grading::apply(image, &settings.color_grading),
        DevelopStage::RadialMasks => radial_masks::apply(image, &settings.radial_masks),
        DevelopStage::Effects => effects::apply(image, &settings.effects, context),
    }
}
