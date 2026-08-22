use super::{SettingsError, canonical_signed_degrees, canonical_zero, validate_range};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ColorBandAdjustment {
    pub hue_shift_degrees: f32,
    pub saturation: f32,
    pub luminance: f32,
}

impl ColorBandAdjustment {
    fn validate(&self, path: &str) -> Result<(), SettingsError> {
        validate_range(
            &format!("{path}.hue_shift_degrees"),
            self.hue_shift_degrees,
            -180.0,
            180.0,
        )?;
        validate_range(
            &format!("{path}.saturation"),
            self.saturation,
            -100.0,
            100.0,
        )?;
        validate_range(&format!("{path}.luminance"), self.luminance, -100.0, 100.0)?;
        Ok(())
    }

    fn is_neutral(&self) -> bool {
        self == &Self::default()
    }

    fn canonicalize(&mut self) {
        self.hue_shift_degrees = canonical_signed_degrees(self.hue_shift_degrees);
        self.saturation = canonical_zero(self.saturation);
        self.luminance = canonical_zero(self.luminance);
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ColorMixerSettings {
    pub red: ColorBandAdjustment,
    pub orange: ColorBandAdjustment,
    pub yellow: ColorBandAdjustment,
    pub green: ColorBandAdjustment,
    pub aqua: ColorBandAdjustment,
    pub blue: ColorBandAdjustment,
    pub purple: ColorBandAdjustment,
    pub magenta: ColorBandAdjustment,
}

impl ColorMixerSettings {
    pub fn validate(&self) -> Result<(), SettingsError> {
        for (name, band) in [
            ("red", &self.red),
            ("orange", &self.orange),
            ("yellow", &self.yellow),
            ("green", &self.green),
            ("aqua", &self.aqua),
            ("blue", &self.blue),
            ("purple", &self.purple),
            ("magenta", &self.magenta),
        ] {
            band.validate(&format!("color_mixer.{name}"))?;
        }
        Ok(())
    }

    pub fn is_neutral(&self) -> bool {
        [
            &self.red,
            &self.orange,
            &self.yellow,
            &self.green,
            &self.aqua,
            &self.blue,
            &self.purple,
            &self.magenta,
        ]
        .into_iter()
        .all(ColorBandAdjustment::is_neutral)
    }

    pub(crate) fn canonicalize(&mut self) {
        for band in [
            &mut self.red,
            &mut self.orange,
            &mut self.yellow,
            &mut self.green,
            &mut self.aqua,
            &mut self.blue,
            &mut self.purple,
            &mut self.magenta,
        ] {
            band.canonicalize();
        }
    }
}
