use super::{SettingsError, validate_range};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CurvePoint {
    pub x: f32,
    pub y: f32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ToneCurve {
    pub points: Vec<CurvePoint>,
}

impl Default for ToneCurve {
    fn default() -> Self {
        Self {
            points: vec![CurvePoint { x: 0.0, y: 0.0 }, CurvePoint { x: 1.0, y: 1.0 }],
        }
    }
}

impl ToneCurve {
    fn validate(&self, path: &str) -> Result<(), SettingsError> {
        if !(2..=32).contains(&self.points.len()) {
            return Err(SettingsError::new(
                format!("{path}.points"),
                "must contain between 2 and 32 points",
            ));
        }
        let mut previous_x = -1.0;
        for (index, point) in self.points.iter().enumerate() {
            validate_range(&format!("{path}.points[{index}].x"), point.x, 0.0, 1.0)?;
            validate_range(&format!("{path}.points[{index}].y"), point.y, 0.0, 1.0)?;
            if point.x <= previous_x {
                return Err(SettingsError::new(
                    format!("{path}.points[{index}].x"),
                    "x coordinates must be strictly increasing",
                ));
            }
            previous_x = point.x;
        }
        if self.points.first().map(|point| point.x) != Some(0.0)
            || self.points.last().map(|point| point.x) != Some(1.0)
        {
            return Err(SettingsError::new(
                format!("{path}.points"),
                "curve must span x=0 through x=1",
            ));
        }
        Ok(())
    }

    fn is_neutral(&self) -> bool {
        self.points.iter().all(|point| point.x == point.y)
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ToneCurvesSettings {
    pub master: ToneCurve,
    pub red: ToneCurve,
    pub green: ToneCurve,
    pub blue: ToneCurve,
}

impl ToneCurvesSettings {
    pub fn validate(&self) -> Result<(), SettingsError> {
        self.master.validate("tone_curves.master")?;
        self.red.validate("tone_curves.red")?;
        self.green.validate("tone_curves.green")?;
        self.blue.validate("tone_curves.blue")?;
        Ok(())
    }

    pub fn is_neutral(&self) -> bool {
        self.master.is_neutral()
            && self.red.is_neutral()
            && self.green.is_neutral()
            && self.blue.is_neutral()
    }
}
