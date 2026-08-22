use super::{SettingsError, validate_range};
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
        validate_range(&format!("{path}.hue_degrees"), self.hue_degrees, 0.0, 360.0)?;
        validate_range(&format!("{path}.saturation"), self.saturation, 0.0, 100.0)?;
        validate_range(&format!("{path}.luminance"), self.luminance, -100.0, 100.0)?;
        Ok(())
    }

    fn is_neutral(&self) -> bool {
        self.saturation == 0.0 && self.luminance == 0.0
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
        validate_range("color_grading.blending", self.blending, 0.0, 100.0)?;
        validate_range("color_grading.balance", self.balance, -100.0, 100.0)?;
        Ok(())
    }

    pub fn is_neutral(&self) -> bool {
        self.shadows.is_neutral()
            && self.midtones.is_neutral()
            && self.highlights.is_neutral()
            && self.balance == 0.0
    }
}
