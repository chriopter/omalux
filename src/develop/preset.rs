use super::{DevelopSettings, SettingsError};
use serde::{Deserialize, Serialize};
use std::fmt;

pub const PRESET_SCHEMA_VERSION: u32 = 2;
const LEGACY_PRESET_SCHEMA_VERSION: u32 = 1;
pub const PRESET_SCHEMA_ID: &str = "io.omacom.grainroom.preset";

#[derive(Deserialize)]
struct PresetEnvelope {
    schema: String,
    schema_version: u32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PresetDocument {
    pub schema: String,
    pub schema_version: u32,
    pub id: String,
    pub name: String,
    pub settings: DevelopSettings,
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
        if envelope.schema != PRESET_SCHEMA_ID {
            return Err(PresetError::UnsupportedSchema(envelope.schema));
        }
        if !matches!(
            envelope.schema_version,
            LEGACY_PRESET_SCHEMA_VERSION | PRESET_SCHEMA_VERSION
        ) {
            return Err(PresetError::UnsupportedVersion(envelope.schema_version));
        }
        let value: serde_json::Value = serde_json::from_str(json).map_err(PresetError::Json)?;
        let mut document: Self = serde_json::from_str(json).map_err(PresetError::Json)?;
        if envelope.schema_version == LEGACY_PRESET_SCHEMA_VERSION {
            validate_v1_semantics(&value, &document)?;
            document.schema_version = PRESET_SCHEMA_VERSION;
        } else if value.pointer("/settings/basics/exposure_ev").is_none() {
            return Err(PresetError::MissingRequiredField(
                "settings.basics.exposure_ev",
            ));
        }
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

fn validate_v1_semantics(
    value: &serde_json::Value,
    document: &PresetDocument,
) -> Result<(), PresetError> {
    if value.pointer("/settings/basics/exposure_ev").is_some() {
        return Err(PresetError::FieldNotAvailable {
            version: LEGACY_PRESET_SCHEMA_VERSION,
            path: "settings.basics.exposure_ev",
        });
    }
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
