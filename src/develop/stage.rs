use serde::{Deserialize, Serialize};

/// Stable identifiers for the canonical develop pipeline.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DevelopStage {
    Geometry,
    Basics,
    ToneCurves,
    ColorMixer,
    ColorGrading,
    RadialMasks,
    Effects,
}

/// The order is part of preset rendering semantics and must only change with a
/// deliberate schema/pipeline migration.
pub const CANONICAL_STAGE_ORDER: [DevelopStage; 7] = [
    DevelopStage::Geometry,
    DevelopStage::Basics,
    DevelopStage::ToneCurves,
    DevelopStage::ColorMixer,
    DevelopStage::ColorGrading,
    DevelopStage::RadialMasks,
    DevelopStage::Effects,
];
