mod basics;
mod color_grading;
mod color_mixer;
mod effects;
mod geometry;
mod radial_masks;
mod tone_curves;

use super::{CpuImage, DevelopSettings, DevelopStage, PipelineError};

pub(super) fn apply(
    stage: DevelopStage,
    image: &mut CpuImage,
    settings: &DevelopSettings,
) -> Result<(), PipelineError> {
    match stage {
        DevelopStage::Geometry => geometry::apply(image, &settings.geometry),
        DevelopStage::Basics => basics::apply(image, &settings.basics),
        DevelopStage::ToneCurves => tone_curves::apply(image, &settings.tone_curves),
        DevelopStage::ColorMixer => color_mixer::apply(image, &settings.color_mixer),
        DevelopStage::ColorGrading => color_grading::apply(image, &settings.color_grading),
        DevelopStage::RadialMasks => radial_masks::apply(image, &settings.radial_masks),
        DevelopStage::Effects => effects::apply(image, &settings.effects),
    }
}
