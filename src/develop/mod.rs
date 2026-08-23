//! Stable, UI-independent foundation for photo development.
//!
//! The modules below deliberately separate persisted settings, parameter
//! metadata, and pixel processing. QML is a client of this API rather than the
//! owner of edit state.

pub mod catalog;
mod color;
mod context;
mod image;
mod orientation;
pub mod overrides;
pub mod parameters;
pub mod pipeline;
pub mod preset;
pub mod settings;
mod stage;

pub use catalog::{MAX_EXTERNAL_PRESET_BYTES, PresetCatalog, PresetCatalogError, load_preset_file};
pub use context::{DevelopRenderContext, ResolvedGrainSeed};
pub use image::{CpuImage, ImageError, PixelChannel, PixelError, RgbaPixel};
pub use overrides::{
    ParameterOverride, ParameterOverrideError, ParameterOverrideValue, apply_parameter_overrides,
    parse_parameter_override,
};
pub use parameters::{
    NeutralRepresentation, ParameterDefinition, ParameterKind, ParameterUnit, parameter_registry,
};
pub use pipeline::{
    DevelopPipeline, DevelopWorkingSetEstimate, DevelopWorkingSetProfile, PipelineError,
    estimate_develop_working_set,
};
pub use preset::{PRESET_SCHEMA_ID, PRESET_SCHEMA_VERSION, PresetDocument, PresetError};
pub use settings::{
    BasicsSettings, ColorBandAdjustment, ColorGradeRange, ColorGradingSettings, ColorMixerSettings,
    CropRect, CurvePoint, DevelopSettings, EffectsSettings, GeometrySettings, GrainSettings,
    LocalAdjustments, RadialMask, RadialMasksSettings, SettingsError, ToneCurve,
    ToneCurvesSettings,
};
pub use stage::{CANONICAL_STAGE_ORDER, DevelopStage};
