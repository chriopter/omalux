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
pub use tone_curves::{CurvePoint, TONE_CURVE_MAX, TONE_CURVE_MIN, ToneCurve, ToneCurvesSettings};

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
    pub(crate) fn try_clone(&self) -> Result<Self, ()> {
        fn clone_curve(curve: &ToneCurve) -> Result<ToneCurve, ()> {
            let mut points = Vec::new();
            points
                .try_reserve_exact(curve.points.len())
                .map_err(|_| ())?;
            points.extend_from_slice(&curve.points);
            Ok(ToneCurve { points })
        }

        let mut masks = Vec::new();
        masks
            .try_reserve_exact(self.radial_masks.masks.len())
            .map_err(|_| ())?;
        for mask in &self.radial_masks.masks {
            let mut id = String::new();
            id.try_reserve_exact(mask.id.len()).map_err(|_| ())?;
            id.push_str(&mask.id);
            masks.push(RadialMask {
                id,
                enabled: mask.enabled,
                center_x: mask.center_x,
                center_y: mask.center_y,
                radius_x: mask.radius_x,
                radius_y: mask.radius_y,
                rotation_degrees: mask.rotation_degrees,
                feather: mask.feather,
                opacity: mask.opacity,
                invert: mask.invert,
                adjustments: mask.adjustments.clone(),
            });
        }
        Ok(Self {
            geometry: self.geometry.clone(),
            basics: self.basics.clone(),
            tone_curves: ToneCurvesSettings {
                master: clone_curve(&self.tone_curves.master)?,
                red: clone_curve(&self.tone_curves.red)?,
                green: clone_curve(&self.tone_curves.green)?,
                blue: clone_curve(&self.tone_curves.blue)?,
            },
            color_mixer: self.color_mixer.clone(),
            color_grading: self.color_grading.clone(),
            effects: self.effects.clone(),
            radial_masks: RadialMasksSettings { masks },
        })
    }

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

    pub fn canonicalize(&mut self) {
        self.geometry.canonicalize();
        self.basics.canonicalize();
        self.tone_curves.canonicalize();
        self.color_mixer.canonicalize();
        self.color_grading.canonicalize();
        self.effects.canonicalize();
        self.radial_masks.canonicalize();
    }

    pub fn canonicalized(&self) -> Self {
        let mut settings = self.clone();
        settings.canonicalize();
        settings
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

/// Builds a dynamic error path only on failure. Settings validation is part of
/// bounded-render preflight and its successful path must not allocate.
pub(crate) fn validate_range_lazy<F>(
    path: F,
    value: f32,
    minimum: f32,
    maximum: f32,
) -> Result<(), SettingsError>
where
    F: FnOnce() -> String,
{
    if value.is_finite() && (minimum..=maximum).contains(&value) {
        return Ok(());
    }
    validate_range(&path(), value, minimum, maximum)
}

pub(crate) fn canonical_zero(value: f32) -> f32 {
    if value == 0.0 { 0.0 } else { value }
}

pub(crate) fn canonical_unsigned_degrees(value: f32) -> f32 {
    canonical_zero(value.rem_euclid(360.0))
}

pub(crate) fn canonical_signed_degrees(value: f32) -> f32 {
    canonical_zero((value + 180.0).rem_euclid(360.0) - 180.0)
}
