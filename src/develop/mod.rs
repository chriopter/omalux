//! Stable, UI-independent foundation for photo development.
//!
//! The modules below deliberately separate persisted settings, parameter
//! metadata, and pixel processing. QML is a client of this API rather than the
//! owner of edit state.

mod color;
mod image;
mod orientation;
pub mod parameters;
pub mod pipeline;
pub mod preset;
pub mod settings;
mod stage;

pub use image::{CpuImage, ImageError, PixelChannel, PixelError, RgbaPixel};
pub use parameters::{
    NeutralRepresentation, ParameterDefinition, ParameterKind, ParameterUnit, parameter_registry,
};
pub use pipeline::{DevelopPipeline, PipelineError};
pub use preset::{PRESET_SCHEMA_ID, PRESET_SCHEMA_VERSION, PresetDocument, PresetError};
pub use settings::{
    BasicsSettings, ColorBandAdjustment, ColorGradeRange, ColorGradingSettings, ColorMixerSettings,
    CropRect, CurvePoint, DevelopSettings, EffectsSettings, GeometrySettings, GrainSettings,
    LocalAdjustments, RadialMask, RadialMasksSettings, SettingsError, ToneCurve,
    ToneCurvesSettings,
};
pub use stage::{CANONICAL_STAGE_ORDER, DevelopStage};
