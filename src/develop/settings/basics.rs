use super::{SettingsError, validate_range};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BasicsSettings {
    pub brightness: f32,
    pub contrast: f32,
    pub clarity: f32,
    pub highlights: f32,
    pub shadows: f32,
    pub whites: f32,
    pub blacks: f32,
    pub saturation: f32,
    pub vibrance: f32,
    pub temperature: f32,
    pub tint: f32,
}

impl BasicsSettings {
    pub fn validate(&self) -> Result<(), SettingsError> {
        for (name, value) in [
            ("brightness", self.brightness),
            ("contrast", self.contrast),
            ("clarity", self.clarity),
            ("highlights", self.highlights),
            ("shadows", self.shadows),
            ("whites", self.whites),
            ("blacks", self.blacks),
            ("saturation", self.saturation),
            ("vibrance", self.vibrance),
            ("temperature", self.temperature),
            ("tint", self.tint),
        ] {
            validate_range(&format!("basics.{name}"), value, -100.0, 100.0)?;
        }
        Ok(())
    }

    pub fn is_neutral(&self) -> bool {
        self == &Self::default()
    }
}
