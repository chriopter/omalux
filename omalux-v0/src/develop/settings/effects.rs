use super::{SettingsError, canonical_zero, validate_range};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GrainSettings {
    pub amount: f32,
    pub size_iso: f32,
    pub midtone_response: f32,
}

impl Default for GrainSettings {
    fn default() -> Self {
        Self {
            amount: 0.0,
            size_iso: 4000.0,
            midtone_response: 100.0,
        }
    }
}

impl GrainSettings {
    fn validate(&self) -> Result<(), SettingsError> {
        validate_range("effects.grain.amount", self.amount, 0.0, 150.0)?;
        validate_range("effects.grain.size_iso", self.size_iso, 20.0, 12800.0)?;
        validate_range(
            "effects.grain.midtone_response",
            self.midtone_response,
            0.0,
            100.0,
        )?;
        Ok(())
    }

    fn is_neutral(&self) -> bool {
        self.amount == 0.0
    }

    fn canonicalize(&mut self) {
        self.amount = canonical_zero(self.amount);
        self.size_iso = canonical_zero(self.size_iso);
        self.midtone_response = canonical_zero(self.midtone_response);
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EffectsSettings {
    /// Sensor noise reduction, split because luminance and colour noise need
    /// very different amounts: colour noise carries nothing worth keeping and
    /// can be smoothed hard, while luminance noise sits on the same scale as
    /// fine detail.
    #[serde(default, skip_serializing_if = "crate::develop::settings::is_zero")]
    pub luminance_noise_reduction: f32,
    #[serde(default, skip_serializing_if = "crate::develop::settings::is_zero")]
    pub colour_noise_reduction: f32,
    pub bloom: f32,
    pub halation: f32,
    pub fade: f32,
    pub vignette: f32,
    pub sharpness: f32,
    pub grain: GrainSettings,
}

impl Default for EffectsSettings {
    fn default() -> Self {
        Self {
            luminance_noise_reduction: 0.0,
            colour_noise_reduction: 0.0,
            bloom: 0.0,
            halation: 0.0,
            fade: 0.0,
            vignette: 0.0,
            sharpness: 0.0,
            grain: GrainSettings::default(),
        }
    }
}

impl EffectsSettings {
    pub fn validate(&self) -> Result<(), SettingsError> {
        validate_range(
            "effects.luminance_noise_reduction",
            self.luminance_noise_reduction,
            0.0,
            100.0,
        )?;
        validate_range(
            "effects.colour_noise_reduction",
            self.colour_noise_reduction,
            0.0,
            100.0,
        )?;
        validate_range("effects.bloom", self.bloom, 0.0, 200.0)?;
        validate_range("effects.halation", self.halation, 0.0, 200.0)?;
        validate_range("effects.fade", self.fade, 0.0, 200.0)?;
        validate_range("effects.vignette", self.vignette, -150.0, 150.0)?;
        validate_range("effects.sharpness", self.sharpness, 0.0, 150.0)?;
        self.grain.validate()?;
        Ok(())
    }

    pub fn is_neutral(&self) -> bool {
        self.luminance_noise_reduction == 0.0
            && self.colour_noise_reduction == 0.0
            && self.bloom == 0.0
            && self.halation == 0.0
            && self.fade == 0.0
            && self.vignette == 0.0
            && self.sharpness == 0.0
            && self.grain.is_neutral()
    }

    pub(crate) fn canonicalize(&mut self) {
        self.luminance_noise_reduction = canonical_zero(self.luminance_noise_reduction);
        self.colour_noise_reduction = canonical_zero(self.colour_noise_reduction);
        self.bloom = canonical_zero(self.bloom);
        self.halation = canonical_zero(self.halation);
        self.fade = canonical_zero(self.fade);
        self.vignette = canonical_zero(self.vignette);
        self.sharpness = canonical_zero(self.sharpness);
        self.grain.canonicalize();
    }
}
