use super::{SettingsError, canonical_signed_degrees, canonical_zero, validate_range_lazy};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LocalAdjustments {
    pub exposure_ev: f32,
    pub brightness: f32,
    pub contrast: f32,
    pub saturation: f32,
    pub temperature: f32,
    pub tint: f32,
    pub sharpness: f32,
}

impl LocalAdjustments {
    fn validate(&self, index: usize) -> Result<(), SettingsError> {
        validate_range_lazy(
            || format!("radial_masks.masks[{index}].adjustments.exposure_ev"),
            self.exposure_ev,
            -5.0,
            5.0,
        )?;
        for (name, value, minimum, maximum) in [
            ("brightness", self.brightness, -300.0, 300.0),
            ("contrast", self.contrast, -200.0, 200.0),
            ("saturation", self.saturation, -100.0, 200.0),
            ("temperature", self.temperature, -150.0, 150.0),
            ("tint", self.tint, -150.0, 150.0),
            ("sharpness", self.sharpness, -150.0, 150.0),
        ] {
            validate_range_lazy(
                || format!("radial_masks.masks[{index}].adjustments.{name}"),
                value,
                minimum,
                maximum,
            )?;
        }
        Ok(())
    }

    fn is_neutral(&self) -> bool {
        self == &Self::default()
    }

    fn canonicalize(&mut self) {
        self.exposure_ev = canonical_zero(self.exposure_ev);
        self.brightness = canonical_zero(self.brightness);
        self.contrast = canonical_zero(self.contrast);
        self.saturation = canonical_zero(self.saturation);
        self.temperature = canonical_zero(self.temperature);
        self.tint = canonical_zero(self.tint);
        self.sharpness = canonical_zero(self.sharpness);
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RadialMask {
    pub id: String,
    pub enabled: bool,
    pub center_x: f32,
    pub center_y: f32,
    pub radius_x: f32,
    pub radius_y: f32,
    pub rotation_degrees: f32,
    pub feather: f32,
    pub opacity: f32,
    pub invert: bool,
    pub adjustments: LocalAdjustments,
}

impl RadialMask {
    fn validate(&self, index: usize) -> Result<(), SettingsError> {
        if self.id.is_empty() || self.id.len() > 64 {
            return Err(SettingsError::new(
                format!("radial_masks.masks[{index}].id"),
                "must contain between 1 and 64 bytes",
            ));
        }
        validate_range_lazy(
            || format!("radial_masks.masks[{index}].center_x"),
            self.center_x,
            0.0,
            1.0,
        )?;
        validate_range_lazy(
            || format!("radial_masks.masks[{index}].center_y"),
            self.center_y,
            0.0,
            1.0,
        )?;
        validate_range_lazy(
            || format!("radial_masks.masks[{index}].radius_x"),
            self.radius_x,
            f32::EPSILON,
            2.0,
        )?;
        validate_range_lazy(
            || format!("radial_masks.masks[{index}].radius_y"),
            self.radius_y,
            f32::EPSILON,
            2.0,
        )?;
        validate_range_lazy(
            || format!("radial_masks.masks[{index}].rotation_degrees"),
            self.rotation_degrees,
            -180.0,
            180.0,
        )?;
        validate_range_lazy(
            || format!("radial_masks.masks[{index}].feather"),
            self.feather,
            0.0,
            1.0,
        )?;
        validate_range_lazy(
            || format!("radial_masks.masks[{index}].opacity"),
            self.opacity,
            0.0,
            1.0,
        )?;
        self.adjustments.validate(index)?;
        Ok(())
    }

    fn is_neutral(&self) -> bool {
        !self.enabled || self.opacity == 0.0 || self.adjustments.is_neutral()
    }

    fn canonicalize(&mut self) {
        self.center_x = canonical_zero(self.center_x);
        self.center_y = canonical_zero(self.center_y);
        self.radius_x = canonical_zero(self.radius_x);
        self.radius_y = canonical_zero(self.radius_y);
        self.rotation_degrees = canonical_signed_degrees(self.rotation_degrees);
        self.feather = canonical_zero(self.feather);
        self.opacity = canonical_zero(self.opacity);
        self.adjustments.canonicalize();
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RadialMasksSettings {
    pub masks: Vec<RadialMask>,
}

impl RadialMasksSettings {
    pub fn validate(&self) -> Result<(), SettingsError> {
        if self.masks.len() > 64 {
            return Err(SettingsError::new(
                "radial_masks.masks",
                "must not contain more than 64 masks",
            ));
        }
        for (index, mask) in self.masks.iter().enumerate() {
            mask.validate(index)?;
            if self.masks[..index]
                .iter()
                .any(|previous| previous.id == mask.id)
            {
                return Err(SettingsError::new(
                    format!("radial_masks.masks[{index}].id"),
                    "mask ids must be unique",
                ));
            }
        }
        Ok(())
    }

    pub fn is_neutral(&self) -> bool {
        self.masks.iter().all(RadialMask::is_neutral)
    }

    pub(crate) fn canonicalize(&mut self) {
        for mask in &mut self.masks {
            mask.canonicalize();
        }
    }
}
