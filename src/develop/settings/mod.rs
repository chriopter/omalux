mod basics;
mod color_grading;
mod color_mixer;
mod effects;
mod geometry;
mod masks;
mod tone_curves;

pub use basics::BasicsSettings;
pub use color_grading::{ColorGradeRange, ColorGradingSettings};
pub use color_mixer::{ColorBandAdjustment, ColorMixerSettings};
pub use effects::{EffectsSettings, GrainSettings};
pub use geometry::{CropRect, GeometrySettings};
pub use masks::{LocalAdjustments, RadialMask, RadialMasksSettings};
use serde::{Deserialize, Serialize};
use std::fmt;
pub use tone_curves::{CurvePoint, ToneCurve, ToneCurvesSettings};

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DevelopSettings {
    pub geometry: GeometrySettings,
    pub basics: BasicsSettings,
    pub tone_curves: ToneCurvesSettings,
    pub color_mixer: ColorMixerSettings,
    pub color_grading: ColorGradingSettings,
    pub effects: EffectsSettings,
    pub radial_masks: RadialMasksSettings,
}

impl DevelopSettings {
    pub fn validate(&self) -> Result<(), SettingsError> {
        self.geometry.validate()?;
        self.basics.validate()?;
        self.tone_curves.validate()?;
        self.color_mixer.validate()?;
        self.color_grading.validate()?;
        self.effects.validate()?;
        self.radial_masks.validate()?;
        Ok(())
    }

    pub fn is_neutral(&self) -> bool {
        self.geometry.is_neutral()
            && self.basics.is_neutral()
            && self.tone_curves.is_neutral()
            && self.color_mixer.is_neutral()
            && self.color_grading.is_neutral()
            && self.effects.is_neutral()
            && self.radial_masks.is_neutral()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SettingsError {
    path: String,
    message: String,
}

impl SettingsError {
    pub(crate) fn new(path: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            path: path.into(),
            message: message.into(),
        }
    }

    pub fn path(&self) -> &str {
        &self.path
    }

    pub fn message(&self) -> &str {
        &self.message
    }
}

impl fmt::Display for SettingsError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}: {}", self.path, self.message)
    }
}

impl std::error::Error for SettingsError {}

pub(crate) fn validate_range(
    path: &str,
    value: f32,
    minimum: f32,
    maximum: f32,
) -> Result<(), SettingsError> {
    if !value.is_finite() {
        return Err(SettingsError::new(path, "must be finite"));
    }
    if !(minimum..=maximum).contains(&value) {
        return Err(SettingsError::new(
            path,
            format!("must be between {minimum} and {maximum}"),
        ));
    }
    Ok(())
}
