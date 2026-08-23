use super::{SettingsError, canonical_zero, validate_range_lazy};
use serde::{Deserialize, Serialize};

pub const TONE_CURVE_MIN: f32 = -4.0;
pub const TONE_CURVE_MAX: f32 = 4.0;

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
        let mut previous_x = f32::NEG_INFINITY;
        let mut previous_y = f32::NEG_INFINITY;
        for (index, point) in self.points.iter().enumerate() {
            validate_range_lazy(
                || format!("{path}.points[{index}].x"),
                point.x,
                TONE_CURVE_MIN,
                TONE_CURVE_MAX,
            )?;
            validate_range_lazy(
                || format!("{path}.points[{index}].y"),
                point.y,
                TONE_CURVE_MIN,
                TONE_CURVE_MAX,
            )?;
            if point.x <= previous_x {
                return Err(SettingsError::new(
                    format!("{path}.points[{index}].x"),
                    "x coordinates must be strictly increasing",
                ));
            }
            if point.y < previous_y {
                return Err(SettingsError::new(
                    format!("{path}.points[{index}].y"),
                    "y coordinates must be nondecreasing for a monotone curve",
                ));
            }
            previous_x = point.x;
            previous_y = point.y;
        }
        if self.points.first().is_none_or(|point| point.x > 0.0)
            || self.points.last().is_none_or(|point| point.x < 1.0)
        {
            return Err(SettingsError::new(
                format!("{path}.points"),
                "curve domain must include x=0 through x=1",
            ));
        }
        Ok(())
    }

    fn is_neutral(&self) -> bool {
        self.points.iter().all(|point| point.x == point.y)
    }

    fn canonicalize(&mut self) {
        for point in &mut self.points {
            point.x = canonical_zero(point.x);
            point.y = canonical_zero(point.y);
        }
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

    pub(crate) fn canonicalize(&mut self) {
        self.master.canonicalize();
        self.red.canonicalize();
        self.green.canonicalize();
        self.blue.canonicalize();
    }
}
