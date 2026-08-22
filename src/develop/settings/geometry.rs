use super::{SettingsError, validate_range};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CropRect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

impl CropRect {
    fn validate(&self) -> Result<(), SettingsError> {
        validate_range("geometry.crop.x", self.x, 0.0, 1.0)?;
        validate_range("geometry.crop.y", self.y, 0.0, 1.0)?;
        validate_range("geometry.crop.width", self.width, f32::EPSILON, 1.0)?;
        validate_range("geometry.crop.height", self.height, f32::EPSILON, 1.0)?;
        if self.x + self.width > 1.0 + f32::EPSILON {
            return Err(SettingsError::new(
                "geometry.crop.width",
                "crop exceeds the right image edge",
            ));
        }
        if self.y + self.height > 1.0 + f32::EPSILON {
            return Err(SettingsError::new(
                "geometry.crop.height",
                "crop exceeds the bottom image edge",
            ));
        }
        Ok(())
    }

    fn is_full_image(&self) -> bool {
        self.x == 0.0 && self.y == 0.0 && self.width == 1.0 && self.height == 1.0
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GeometrySettings {
    pub crop: Option<CropRect>,
    pub quarter_turns_clockwise: u8,
    pub straighten_degrees: f32,
    pub perspective_horizontal: f32,
    pub perspective_vertical: f32,
    pub flip_horizontal: bool,
    pub flip_vertical: bool,
}

impl Default for GeometrySettings {
    fn default() -> Self {
        Self {
            crop: None,
            quarter_turns_clockwise: 0,
            straighten_degrees: 0.0,
            perspective_horizontal: 0.0,
            perspective_vertical: 0.0,
            flip_horizontal: false,
            flip_vertical: false,
        }
    }
}

impl GeometrySettings {
    pub fn validate(&self) -> Result<(), SettingsError> {
        if self.quarter_turns_clockwise > 3 {
            return Err(SettingsError::new(
                "geometry.quarter_turns_clockwise",
                "must be between 0 and 3",
            ));
        }
        validate_range(
            "geometry.straighten_degrees",
            self.straighten_degrees,
            -45.0,
            45.0,
        )?;
        validate_range(
            "geometry.perspective_horizontal",
            self.perspective_horizontal,
            -100.0,
            100.0,
        )?;
        validate_range(
            "geometry.perspective_vertical",
            self.perspective_vertical,
            -100.0,
            100.0,
        )?;
        if let Some(crop) = &self.crop {
            crop.validate()?;
        }
        Ok(())
    }

    pub fn is_neutral(&self) -> bool {
        self.crop.as_ref().is_none_or(CropRect::is_full_image)
            && self.quarter_turns_clockwise == 0
            && self.straighten_degrees == 0.0
            && self.perspective_horizontal == 0.0
            && self.perspective_vertical == 0.0
            && !self.flip_horizontal
            && !self.flip_vertical
    }
}
