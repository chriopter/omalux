use super::{
    SettingsError, canonical_unsigned_degrees, canonical_zero, validate_range, validate_range_lazy,
};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ColorGradeRange {
    pub hue_degrees: f32,
    pub saturation: f32,
    pub luminance: f32,
}

impl ColorGradeRange {
    fn validate(&self, path: &str) -> Result<(), SettingsError> {
        validate_range_lazy(
            || format!("{path}.hue_degrees"),
            self.hue_degrees,
            0.0,
            360.0,
        )?;
        validate_range_lazy(|| format!("{path}.saturation"), self.saturation, 0.0, 200.0)?;
        validate_range_lazy(
            || format!("{path}.luminance"),
            self.luminance,
            -150.0,
            150.0,
        )?;
        Ok(())
    }

    fn is_neutral(&self) -> bool {
        self.saturation == 0.0 && self.luminance == 0.0
    }

    fn canonicalize(&mut self) {
        self.hue_degrees = canonical_unsigned_degrees(self.hue_degrees);
        self.saturation = canonical_zero(self.saturation);
        self.luminance = canonical_zero(self.luminance);
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ColorGradingSettings {
    pub shadows: ColorGradeRange,
    pub midtones: ColorGradeRange,
    pub highlights: ColorGradeRange,
    pub blending: f32,
    pub balance: f32,
}

impl ColorGradingSettings {
    pub fn validate(&self) -> Result<(), SettingsError> {
        self.shadows.validate("color_grading.shadows")?;
        self.midtones.validate("color_grading.midtones")?;
        self.highlights.validate("color_grading.highlights")?;
        validate_range("color_grading.blending", self.blending, 0.0, 150.0)?;
        validate_range("color_grading.balance", self.balance, -150.0, 150.0)?;
        Ok(())
    }

    pub fn is_neutral(&self) -> bool {
        self.shadows.is_neutral() && self.midtones.is_neutral() && self.highlights.is_neutral()
    }

    pub(crate) fn canonicalize(&mut self) {
        self.shadows.canonicalize();
        self.midtones.canonicalize();
        self.highlights.canonicalize();
        self.blending = canonical_zero(self.blending);
        self.balance = canonical_zero(self.balance);
    }
}
