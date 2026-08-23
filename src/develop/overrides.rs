use super::{CropRect, DevelopSettings, ParameterKind, SettingsError, parameter_registry};
use std::fmt;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ParameterOverrideValue {
    Scalar(f32),
    Toggle(bool),
}

#[derive(Clone, Debug, PartialEq)]
pub struct ParameterOverride {
    parameter_id: String,
    value: ParameterOverrideValue,
}

impl ParameterOverride {
    pub fn scalar(parameter_id: impl Into<String>, value: f32) -> Self {
        Self {
            parameter_id: parameter_id.into(),
            value: ParameterOverrideValue::Scalar(value),
        }
    }

    pub fn toggle(parameter_id: impl Into<String>, value: bool) -> Self {
        Self {
            parameter_id: parameter_id.into(),
            value: ParameterOverrideValue::Toggle(value),
        }
    }

    pub fn parameter_id(&self) -> &str {
        &self.parameter_id
    }

    pub fn value(&self) -> ParameterOverrideValue {
        self.value
    }
}

/// Parses one strict `parameter.id=value` CLI expression against the stable
/// parameter registry. Structured values are intentionally JSON-only.
pub fn parse_parameter_override(
    expression: &str,
) -> Result<ParameterOverride, ParameterOverrideError> {
    let (id, raw) = expression
        .split_once('=')
        .filter(|(id, raw)| !id.is_empty() && !raw.is_empty() && !raw.contains('='))
        .ok_or(ParameterOverrideError::InvalidExpression)?;
    let definition = parameter_registry()
        .into_iter()
        .find(|definition| definition.id == id)
        .ok_or_else(|| ParameterOverrideError::UnknownParameter(id.to_owned()))?;
    if is_structured_parameter(id)
        || !matches!(
            definition.kind,
            ParameterKind::Scalar | ParameterKind::Toggle
        )
    {
        return Err(ParameterOverrideError::StructuredParameter(id.to_owned()));
    }
    let value = match definition.kind {
        ParameterKind::Scalar => {
            let value = raw
                .parse::<f32>()
                .map_err(|_| ParameterOverrideError::InvalidScalar(id.to_owned()))?;
            validate_scalar(id, value, definition.minimum, definition.maximum)?;
            ParameterOverrideValue::Scalar(value)
        }
        ParameterKind::Toggle => ParameterOverrideValue::Toggle(match raw {
            "true" => true,
            "false" => false,
            _ => return Err(ParameterOverrideError::InvalidToggle(id.to_owned())),
        }),
        _ => unreachable!("structured kinds returned above"),
    };
    Ok(ParameterOverride {
        parameter_id: id.to_owned(),
        value,
    })
}

/// Applies all overrides transactionally and returns validated canonical
/// settings. Duplicate IDs are rejected rather than depending on CLI order.
pub fn apply_parameter_overrides(
    base: &DevelopSettings,
    overrides: &[ParameterOverride],
) -> Result<DevelopSettings, ParameterOverrideError> {
    base.validate()
        .map_err(ParameterOverrideError::InvalidSettings)?;
    let registry = parameter_registry();
    let mut result = base
        .try_clone()
        .map_err(|_| ParameterOverrideError::Allocation)?;
    for (index, parameter_override) in overrides.iter().enumerate() {
        let id = parameter_override.parameter_id();
        if overrides[..index]
            .iter()
            .any(|previous| previous.parameter_id() == id)
        {
            return Err(ParameterOverrideError::DuplicateParameter(id.to_owned()));
        }
        let definition = registry
            .iter()
            .find(|definition| definition.id == id)
            .ok_or_else(|| ParameterOverrideError::UnknownParameter(id.to_owned()))?;
        if is_structured_parameter(id)
            || !matches!(
                definition.kind,
                ParameterKind::Scalar | ParameterKind::Toggle
            )
        {
            return Err(ParameterOverrideError::StructuredParameter(id.to_owned()));
        }
        match (definition.kind, parameter_override.value()) {
            (ParameterKind::Scalar, ParameterOverrideValue::Scalar(value)) => {
                validate_scalar(id, value, definition.minimum, definition.maximum)?;
                set_scalar(&mut result, id, value)?;
            }
            (ParameterKind::Toggle, ParameterOverrideValue::Toggle(value)) => {
                set_toggle(&mut result, id, value)?;
            }
            _ => return Err(ParameterOverrideError::WrongValueKind(id.to_owned())),
        }
    }
    result
        .validate()
        .map_err(ParameterOverrideError::InvalidSettings)?;
    result.canonicalize();
    result
        .validate()
        .map_err(ParameterOverrideError::InvalidSettings)?;
    Ok(result)
}

fn validate_scalar(
    id: &str,
    value: f32,
    minimum: f32,
    maximum: f32,
) -> Result<(), ParameterOverrideError> {
    if !value.is_finite() || !(minimum..=maximum).contains(&value) {
        return Err(ParameterOverrideError::ScalarOutOfRange {
            parameter_id: id.to_owned(),
            minimum,
            maximum,
        });
    }
    Ok(())
}

fn is_structured_parameter(id: &str) -> bool {
    id.starts_with("tone_curves.")
        || id == "radial_masks"
        || id.starts_with("radial_masks[]")
        || id == "geometry.crop.enabled"
}

fn set_scalar(
    settings: &mut DevelopSettings,
    id: &str,
    value: f32,
) -> Result<(), ParameterOverrideError> {
    match id {
        "geometry.quarter_turns_clockwise" => {
            if value.fract() != 0.0 {
                return Err(ParameterOverrideError::IntegerRequired(id.to_owned()));
            }
            settings.geometry.quarter_turns_clockwise = value as u8;
        }
        "geometry.straighten_degrees" => settings.geometry.straighten_degrees = value,
        "geometry.perspective_horizontal" => settings.geometry.perspective_horizontal = value,
        "geometry.perspective_vertical" => settings.geometry.perspective_vertical = value,
        "geometry.crop.x" => crop(settings).x = value,
        "geometry.crop.y" => crop(settings).y = value,
        "geometry.crop.width" => crop(settings).width = value,
        "geometry.crop.height" => crop(settings).height = value,
        "basics.exposure_ev" => settings.basics.exposure_ev = value,
        "basics.brightness" => settings.basics.brightness = value,
        "basics.contrast" => settings.basics.contrast = value,
        "basics.clarity" => settings.basics.clarity = value,
        "basics.highlights" => settings.basics.highlights = value,
        "basics.shadows" => settings.basics.shadows = value,
        "basics.whites" => settings.basics.whites = value,
        "basics.blacks" => settings.basics.blacks = value,
        "basics.saturation" => settings.basics.saturation = value,
        "basics.vibrance" => settings.basics.vibrance = value,
        "basics.temperature" => settings.basics.temperature = value,
        "basics.tint" => settings.basics.tint = value,
        "color_grading.shadows.hue_degrees" => settings.color_grading.shadows.hue_degrees = value,
        "color_grading.shadows.saturation" => settings.color_grading.shadows.saturation = value,
        "color_grading.shadows.luminance" => settings.color_grading.shadows.luminance = value,
        "color_grading.midtones.hue_degrees" => settings.color_grading.midtones.hue_degrees = value,
        "color_grading.midtones.saturation" => settings.color_grading.midtones.saturation = value,
        "color_grading.midtones.luminance" => settings.color_grading.midtones.luminance = value,
        "color_grading.highlights.hue_degrees" => {
            settings.color_grading.highlights.hue_degrees = value;
        }
        "color_grading.highlights.saturation" => {
            settings.color_grading.highlights.saturation = value;
        }
        "color_grading.highlights.luminance" => {
            settings.color_grading.highlights.luminance = value;
        }
        "color_grading.blending" => settings.color_grading.blending = value,
        "color_grading.balance" => settings.color_grading.balance = value,
        "effects.bloom" => settings.effects.bloom = value,
        "effects.halation" => settings.effects.halation = value,
        "effects.fade" => settings.effects.fade = value,
        "effects.vignette" => settings.effects.vignette = value,
        "effects.sharpness" => settings.effects.sharpness = value,
        "effects.grain.amount" => settings.effects.grain.amount = value,
        "effects.grain.size_iso" => settings.effects.grain.size_iso = value,
        "effects.grain.midtone_response" => settings.effects.grain.midtone_response = value,
        _ if id.starts_with("color_mixer.") => set_color_mixer(settings, id, value)?,
        _ => return Err(ParameterOverrideError::MappingMissing(id.to_owned())),
    }
    Ok(())
}

fn set_toggle(
    settings: &mut DevelopSettings,
    id: &str,
    value: bool,
) -> Result<(), ParameterOverrideError> {
    match id {
        "geometry.flip_horizontal" => settings.geometry.flip_horizontal = value,
        "geometry.flip_vertical" => settings.geometry.flip_vertical = value,
        _ => return Err(ParameterOverrideError::MappingMissing(id.to_owned())),
    }
    Ok(())
}

fn crop(settings: &mut DevelopSettings) -> &mut CropRect {
    settings.geometry.crop.get_or_insert(CropRect {
        x: 0.0,
        y: 0.0,
        width: 1.0,
        height: 1.0,
    })
}

fn set_color_mixer(
    settings: &mut DevelopSettings,
    id: &str,
    value: f32,
) -> Result<(), ParameterOverrideError> {
    let mut components = id.split('.');
    if components.next() != Some("color_mixer") {
        return Err(ParameterOverrideError::MappingMissing(id.to_owned()));
    }
    let band = match components.next() {
        Some("red") => &mut settings.color_mixer.red,
        Some("orange") => &mut settings.color_mixer.orange,
        Some("yellow") => &mut settings.color_mixer.yellow,
        Some("green") => &mut settings.color_mixer.green,
        Some("aqua") => &mut settings.color_mixer.aqua,
        Some("blue") => &mut settings.color_mixer.blue,
        Some("purple") => &mut settings.color_mixer.purple,
        Some("magenta") => &mut settings.color_mixer.magenta,
        _ => return Err(ParameterOverrideError::MappingMissing(id.to_owned())),
    };
    match (components.next(), components.next()) {
        (Some("hue_shift_degrees"), None) => band.hue_shift_degrees = value,
        (Some("saturation"), None) => band.saturation = value,
        (Some("luminance"), None) => band.luminance = value,
        _ => return Err(ParameterOverrideError::MappingMissing(id.to_owned())),
    }
    Ok(())
}

#[derive(Debug)]
pub enum ParameterOverrideError {
    InvalidExpression,
    UnknownParameter(String),
    StructuredParameter(String),
    InvalidScalar(String),
    InvalidToggle(String),
    ScalarOutOfRange {
        parameter_id: String,
        minimum: f32,
        maximum: f32,
    },
    IntegerRequired(String),
    WrongValueKind(String),
    DuplicateParameter(String),
    MappingMissing(String),
    InvalidSettings(SettingsError),
    Allocation,
}

impl fmt::Display for ParameterOverrideError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidExpression => formatter.write_str("override must be parameter.id=value"),
            Self::UnknownParameter(id) => write!(formatter, "unknown parameter {id:?}"),
            Self::StructuredParameter(id) => {
                write!(
                    formatter,
                    "parameter {id:?} requires a complete preset JSON document"
                )
            }
            Self::InvalidScalar(id) => write!(formatter, "parameter {id:?} requires a number"),
            Self::InvalidToggle(id) => {
                write!(formatter, "parameter {id:?} requires exactly true or false")
            }
            Self::ScalarOutOfRange {
                parameter_id,
                minimum,
                maximum,
            } => write!(
                formatter,
                "parameter {parameter_id:?} must be finite and between {minimum} and {maximum}"
            ),
            Self::IntegerRequired(id) => write!(formatter, "parameter {id:?} requires an integer"),
            Self::WrongValueKind(id) => {
                write!(formatter, "parameter {id:?} has the wrong value kind")
            }
            Self::DuplicateParameter(id) => {
                write!(formatter, "parameter {id:?} is overridden twice")
            }
            Self::MappingMissing(id) => {
                write!(formatter, "parameter {id:?} has no override mapping")
            }
            Self::InvalidSettings(error) => {
                write!(formatter, "overrides produce invalid settings: {error}")
            }
            Self::Allocation => formatter.write_str("override settings allocation failed"),
        }
    }
}

impl std::error::Error for ParameterOverrideError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::InvalidSettings(error) => Some(error),
            _ => None,
        }
    }
}
