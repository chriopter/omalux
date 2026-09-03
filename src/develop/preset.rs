use super::{DevelopSettings, SettingsError};
use serde::{Deserialize, Deserializer, Serialize, de::Error as _};
use std::fmt;

pub const PRESET_SCHEMA_VERSION: u32 = 3;
const LEGACY_PRESET_SCHEMA_VERSION: u32 = 1;
const PREVIOUS_PRESET_SCHEMA_VERSION: u32 = 2;
pub const PRESET_SCHEMA_ID: &str = "org.omalux.preset";
/// Identity that presets carried before the project moved to its own
/// namespace. Still read, never written: an import is normalized to the
/// current identity.
const LEGACY_PRESET_SCHEMA_ID: &str = "io.omacom.omalux.preset";
const LOCAL_EXPOSURE_PATH: &str = "settings.radial_masks.masks[].adjustments.exposure_ev";

#[derive(Deserialize)]
struct PresetEnvelope {
    schema: String,
    schema_version: u32,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct PresetDocument {
    pub schema: String,
    pub schema_version: u32,
    pub id: String,
    pub name: String,
    pub settings: DevelopSettings,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PresetDocumentWire {
    schema: String,
    schema_version: u32,
    id: String,
    name: String,
    settings: serde_json::Value,
}

impl<'de> Deserialize<'de> for PresetDocument {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let wire = PresetDocumentWire::deserialize(deserializer)?;
        if !is_supported_schema(&wire.schema) {
            return Err(D::Error::custom("unsupported preset schema"));
        }
        if !matches!(
            wire.schema_version,
            LEGACY_PRESET_SCHEMA_VERSION | PREVIOUS_PRESET_SCHEMA_VERSION | PRESET_SCHEMA_VERSION
        ) {
            return Err(D::Error::custom("unsupported preset schema version"));
        }

        let mut settings_value = wire.settings;
        let basics = settings_value
            .get_mut("basics")
            .and_then(serde_json::Value::as_object_mut)
            .ok_or_else(|| D::Error::custom("missing or invalid settings.basics"))?;
        let has_exposure = basics.contains_key("exposure_ev");
        match wire.schema_version {
            LEGACY_PRESET_SCHEMA_VERSION => {
                if has_exposure {
                    return Err(D::Error::custom(
                        "settings.basics.exposure_ev is unavailable in schema v1",
                    ));
                }
                basics.insert("exposure_ev".to_owned(), serde_json::Value::from(0.0));
                migrate_local_exposure(&mut settings_value, LEGACY_PRESET_SCHEMA_VERSION)
                    .map_err(D::Error::custom)?;
            }
            PREVIOUS_PRESET_SCHEMA_VERSION | PRESET_SCHEMA_VERSION if !has_exposure => {
                return Err(D::Error::missing_field("settings.basics.exposure_ev"));
            }
            PREVIOUS_PRESET_SCHEMA_VERSION => {
                migrate_local_exposure(&mut settings_value, PREVIOUS_PRESET_SCHEMA_VERSION)
                    .map_err(D::Error::custom)?;
            }
            PRESET_SCHEMA_VERSION => {
                require_v3_local_exposure(&settings_value).map_err(D::Error::custom)?;
            }
            _ => unreachable!("version checked above"),
        }

        let settings: DevelopSettings =
            serde_json::from_value(settings_value).map_err(D::Error::custom)?;
        let document = Self {
            // Imports are normalized so all newly serialized documents use the
            // current identity, including documents read through the legacy alias.
            schema: PRESET_SCHEMA_ID.to_owned(),
            schema_version: PRESET_SCHEMA_VERSION,
            id: wire.id,
            name: wire.name,
            settings,
        };
        if wire.schema_version == LEGACY_PRESET_SCHEMA_VERSION {
            validate_v1_curve_semantics(&document).map_err(D::Error::custom)?;
        }
        document.validate().map_err(D::Error::custom)?;
        Ok(document)
    }
}

impl PresetDocument {
    pub fn new(id: impl Into<String>, name: impl Into<String>, settings: DevelopSettings) -> Self {
        Self {
            schema: PRESET_SCHEMA_ID.to_owned(),
            schema_version: PRESET_SCHEMA_VERSION,
            id: id.into(),
            name: name.into(),
            settings,
        }
    }

    pub fn validate(&self) -> Result<(), PresetError> {
        if self.schema != PRESET_SCHEMA_ID {
            return Err(PresetError::UnsupportedSchema(self.schema.clone()));
        }
        if self.schema_version != PRESET_SCHEMA_VERSION {
            return Err(PresetError::UnsupportedVersion(self.schema_version));
        }
        if self.id.is_empty()
            || self.id.len() > 96
            || !self
                .id
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
        {
            return Err(PresetError::InvalidIdentity(
                "id must be 1-96 ASCII letters, digits, '.', '-' or '_'".to_owned(),
            ));
        }
        if self.name.trim().is_empty() || self.name.len() > 160 {
            return Err(PresetError::InvalidIdentity(
                "name must contain 1-160 bytes".to_owned(),
            ));
        }
        self.settings.validate().map_err(PresetError::Settings)
    }

    pub fn from_json(json: &str) -> Result<Self, PresetError> {
        let envelope: PresetEnvelope = serde_json::from_str(json).map_err(PresetError::Json)?;
        if !is_supported_schema(&envelope.schema) {
            return Err(PresetError::UnsupportedSchema(envelope.schema));
        }
        if !matches!(
            envelope.schema_version,
            LEGACY_PRESET_SCHEMA_VERSION | PREVIOUS_PRESET_SCHEMA_VERSION | PRESET_SCHEMA_VERSION
        ) {
            return Err(PresetError::UnsupportedVersion(envelope.schema_version));
        }
        let value: serde_json::Value = serde_json::from_str(json).map_err(PresetError::Json)?;
        if envelope.schema_version == LEGACY_PRESET_SCHEMA_VERSION
            && value.pointer("/settings/basics/exposure_ev").is_some()
        {
            return Err(PresetError::FieldNotAvailable {
                version: LEGACY_PRESET_SCHEMA_VERSION,
                path: "settings.basics.exposure_ev",
            });
        }
        if matches!(
            envelope.schema_version,
            LEGACY_PRESET_SCHEMA_VERSION | PREVIOUS_PRESET_SCHEMA_VERSION
        ) && has_any_local_exposure(&value)
        {
            return Err(PresetError::FieldNotAvailable {
                version: envelope.schema_version,
                path: LOCAL_EXPOSURE_PATH,
            });
        }
        if envelope.schema_version == LEGACY_PRESET_SCHEMA_VERSION {
            validate_v1_value_semantics(&value)?;
        }
        if matches!(
            envelope.schema_version,
            PREVIOUS_PRESET_SCHEMA_VERSION | PRESET_SCHEMA_VERSION
        ) && value.pointer("/settings/basics/exposure_ev").is_none()
        {
            return Err(PresetError::MissingRequiredField(
                "settings.basics.exposure_ev",
            ));
        }
        if envelope.schema_version == PRESET_SCHEMA_VERSION && has_missing_local_exposure(&value) {
            return Err(PresetError::MissingRequiredField(LOCAL_EXPOSURE_PATH));
        }
        let document: Self = serde_json::from_str(json).map_err(PresetError::Json)?;
        document.validate()?;
        Ok(document)
    }

    /// Struct field order plus compact serde JSON yields deterministic output.
    /// No setting is omitted, including neutral values.
    pub fn to_canonical_json(&self) -> Result<String, PresetError> {
        self.validate()?;
        let mut canonical = self.clone();
        canonical.settings.canonicalize();
        serde_json::to_string(&canonical).map_err(PresetError::Json)
    }
}

fn is_supported_schema(schema: &str) -> bool {
    schema == PRESET_SCHEMA_ID || schema == LEGACY_PRESET_SCHEMA_ID
}

fn local_adjustments(
    value: &serde_json::Value,
) -> impl Iterator<Item = &serde_json::Map<String, serde_json::Value>> {
    value
        .pointer("/settings/radial_masks/masks")
        .and_then(serde_json::Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|mask| mask.get("adjustments"))
        .filter_map(serde_json::Value::as_object)
}

fn has_any_local_exposure(value: &serde_json::Value) -> bool {
    local_adjustments(value).any(|adjustments| adjustments.contains_key("exposure_ev"))
}

fn has_missing_local_exposure(value: &serde_json::Value) -> bool {
    local_adjustments(value).any(|adjustments| !adjustments.contains_key("exposure_ev"))
}

fn migrate_local_exposure(
    value: &mut serde_json::Value,
    source_version: u32,
) -> Result<(), String> {
    let Some(masks) = value
        .pointer_mut("/radial_masks/masks")
        .and_then(serde_json::Value::as_array_mut)
    else {
        return Ok(());
    };
    for mask in masks {
        let Some(adjustments) = mask
            .get_mut("adjustments")
            .and_then(serde_json::Value::as_object_mut)
        else {
            continue;
        };
        if adjustments.contains_key("exposure_ev") {
            return Err(format!(
                "local exposure is unavailable in schema v{source_version}"
            ));
        }
        adjustments.insert("exposure_ev".to_owned(), serde_json::Value::from(0.0));
    }
    Ok(())
}

fn require_v3_local_exposure(value: &serde_json::Value) -> Result<(), &'static str> {
    if has_missing_local_exposure_in_settings(value) {
        Err("missing field settings.radial_masks.masks[].adjustments.exposure_ev")
    } else {
        Ok(())
    }
}

fn has_missing_local_exposure_in_settings(value: &serde_json::Value) -> bool {
    value
        .pointer("/radial_masks/masks")
        .and_then(serde_json::Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|mask| mask.get("adjustments"))
        .filter_map(serde_json::Value::as_object)
        .any(|adjustments| !adjustments.contains_key("exposure_ev"))
}

fn validate_v1_value_semantics(value: &serde_json::Value) -> Result<(), PresetError> {
    for (path, pointer) in [
        (
            "settings.tone_curves.master",
            "/settings/tone_curves/master/points",
        ),
        (
            "settings.tone_curves.red",
            "/settings/tone_curves/red/points",
        ),
        (
            "settings.tone_curves.green",
            "/settings/tone_curves/green/points",
        ),
        (
            "settings.tone_curves.blue",
            "/settings/tone_curves/blue/points",
        ),
    ] {
        let Some(points) = value.pointer(pointer).and_then(serde_json::Value::as_array) else {
            continue;
        };
        let legacy_range = points.iter().all(|point| {
            let x = point.get("x").and_then(serde_json::Value::as_f64);
            let y = point.get("y").and_then(serde_json::Value::as_f64);
            x.is_none_or(|value| (0.0..=1.0).contains(&value))
                && y.is_none_or(|value| (0.0..=1.0).contains(&value))
        });
        let legacy_domain = points
            .first()
            .and_then(|point| point.get("x"))
            .and_then(serde_json::Value::as_f64)
            .is_none_or(|value| value == 0.0)
            && points
                .last()
                .and_then(|point| point.get("x"))
                .and_then(serde_json::Value::as_f64)
                .is_none_or(|value| value == 1.0);
        if !legacy_range || !legacy_domain {
            return Err(PresetError::FieldNotAvailable {
                version: LEGACY_PRESET_SCHEMA_VERSION,
                path,
            });
        }
    }
    Ok(())
}

fn validate_v1_curve_semantics(document: &PresetDocument) -> Result<(), PresetError> {
    for (path, curve) in [
        (
            "settings.tone_curves.master",
            &document.settings.tone_curves.master,
        ),
        (
            "settings.tone_curves.red",
            &document.settings.tone_curves.red,
        ),
        (
            "settings.tone_curves.green",
            &document.settings.tone_curves.green,
        ),
        (
            "settings.tone_curves.blue",
            &document.settings.tone_curves.blue,
        ),
    ] {
        let legacy_range = curve
            .points
            .iter()
            .all(|point| (0.0..=1.0).contains(&point.x) && (0.0..=1.0).contains(&point.y));
        let legacy_domain = curve.points.first().map(|point| point.x) == Some(0.0)
            && curve.points.last().map(|point| point.x) == Some(1.0);
        if !legacy_range || !legacy_domain {
            return Err(PresetError::FieldNotAvailable {
                version: LEGACY_PRESET_SCHEMA_VERSION,
                path,
            });
        }
    }
    Ok(())
}

#[derive(Debug)]
pub enum PresetError {
    Json(serde_json::Error),
    UnsupportedSchema(String),
    UnsupportedVersion(u32),
    MissingRequiredField(&'static str),
    FieldNotAvailable { version: u32, path: &'static str },
    InvalidIdentity(String),
    Settings(SettingsError),
}

impl fmt::Display for PresetError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Json(error) => write!(formatter, "invalid preset JSON: {error}"),
            Self::UnsupportedSchema(schema) => {
                write!(formatter, "unsupported preset schema {schema:?}")
            }
            Self::UnsupportedVersion(version) => {
                write!(formatter, "unsupported preset schema version {version}")
            }
            Self::MissingRequiredField(path) => {
                write!(formatter, "missing required preset field {path}")
            }
            Self::FieldNotAvailable { version, path } => {
                write!(
                    formatter,
                    "field {path} is not available in preset schema version {version}"
                )
            }
            Self::InvalidIdentity(message) => {
                write!(formatter, "invalid preset identity: {message}")
            }
            Self::Settings(error) => write!(formatter, "invalid preset settings: {error}"),
        }
    }
}

impl std::error::Error for PresetError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Json(error) => Some(error),
            Self::Settings(error) => Some(error),
            _ => None,
        }
    }
}
