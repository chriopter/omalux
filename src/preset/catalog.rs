use super::{PresetDocument, PresetError};
use std::{fmt, fs::File, io::Read, path::Path};

#[cfg(target_os = "linux")]
use rustix::fs::{self, FileType, Mode, OFlags};

/// External preset documents are intentionally small declarative JSON files.
pub const MAX_EXTERNAL_PRESET_BYTES: u64 = 1024 * 1024;

// Keep this manifest explicit: Cargo follows every include at package time and
// reviewers can audit the complete public catalog without build-time scanning.
// `PresetCatalog::from_documents` still sorts by stable ID before exposure.
const BUILTIN_PRESETS: &[&str] = &[
    include_str!("../../presets/builtin/community-amber-grain.json"),
    include_str!("../../presets/builtin/community-daylight-25.json"),
    include_str!("../../presets/builtin/community-desert-signal.json"),
    include_str!("../../presets/builtin/community-honey-hour.json"),
    include_str!("../../presets/builtin/community-muted-alloy.json"),
    include_str!("../../presets/builtin/community-quiet-negative.json"),
    include_str!("../../presets/builtin/community-roseglass.json"),
    include_str!("../../presets/builtin/community-studio-cut.json"),
    include_str!("../../presets/builtin/community-sunwashed-instant.json"),
    include_str!("../../presets/builtin/neutral.json"),
    include_str!("../../presets/builtin/personal-adams-style.json"),
    include_str!("../../presets/builtin/personal-blitz.json"),
    include_str!("../../presets/builtin/personal-blume.json"),
    include_str!("../../presets/builtin/personal-c300-adbvanced.json"),
    include_str!("../../presets/builtin/personal-kerry.json"),
    include_str!("../../presets/builtin/personal-kirche-2.json"),
    include_str!("../../presets/builtin/personal-kirche.json"),
    include_str!("../../presets/builtin/personal-lampe-1.json"),
    include_str!("../../presets/builtin/personal-mantel.json"),
    include_str!("../../presets/builtin/personal-metallbauer.json"),
    include_str!("../../presets/builtin/personal-pflanze.json"),
    include_str!("../../presets/builtin/personal-street.json"),
    include_str!("../../presets/builtin/personal-verbania.json"),
    include_str!("../../presets/builtin/series-alpine-contrast.json"),
    include_str!("../../presets/builtin/series-alpine-cross.json"),
    include_str!("../../presets/builtin/series-alpine-neutral.json"),
    include_str!("../../presets/builtin/series-cedar-deep.json"),
    include_str!("../../presets/builtin/series-cedar-fade.json"),
];

#[derive(Clone, Debug, PartialEq)]
pub struct PresetCatalog {
    documents: Vec<PresetDocument>,
}

impl PresetCatalog {
    /// Parses and validates the preset documents shipped with Omalux.
    ///
    /// Built-ins must already be canonical JSON. This prevents a checked-in
    /// document from changing identity when it is later serialized.
    pub fn built_in() -> Result<Self, PresetCatalogError> {
        let mut documents = Vec::new();
        documents
            .try_reserve_exact(BUILTIN_PRESETS.len())
            .map_err(|_| PresetCatalogError::Allocation)?;
        for json in BUILTIN_PRESETS {
            documents.push(parse_builtin_json(json)?);
        }
        Self::from_documents(documents)
    }

    /// Builds a deterministic catalog from already parsed documents.
    pub fn from_documents(mut documents: Vec<PresetDocument>) -> Result<Self, PresetCatalogError> {
        for document in &mut documents {
            document.validate().map_err(PresetCatalogError::Preset)?;
            document.settings.canonicalize();
            document.validate().map_err(PresetCatalogError::Preset)?;
        }
        documents.sort_by(|left, right| left.id.cmp(&right.id));
        if let Some(pair) = documents.windows(2).find(|pair| pair[0].id == pair[1].id) {
            return Err(PresetCatalogError::DuplicateId(pair[0].id.clone()));
        }
        Ok(Self { documents })
    }

    pub fn documents(&self) -> &[PresetDocument] {
        &self.documents
    }

    pub fn get(&self, id: &str) -> Option<&PresetDocument> {
        self.documents
            .binary_search_by(|document| document.id.as_str().cmp(id))
            .ok()
            .map(|index| &self.documents[index])
    }
}

fn parse_builtin_json(json: &str) -> Result<PresetDocument, PresetCatalogError> {
    let document = PresetDocument::from_json(json).map_err(PresetCatalogError::Preset)?;
    let canonical = document
        .to_canonical_json()
        .map_err(PresetCatalogError::Preset)?;
    // Repository convention: exactly canonical compact JSON followed by one
    // LF. Do not admit spaces, CRLF, or additional blank lines.
    if json.strip_suffix('\n') != Some(canonical.as_str()) {
        return Err(PresetCatalogError::NonCanonical(document.id));
    }
    Ok(document)
}

/// Opens one external preset without following a final symlink, bounds it
/// before and during reading, then delegates all schema checks to
/// `PresetDocument::from_json`.
pub fn load_preset_file(path: &Path) -> Result<PresetDocument, PresetCatalogError> {
    load_preset_file_with_limit(path, MAX_EXTERNAL_PRESET_BYTES)
}

fn load_preset_file_with_limit(
    path: &Path,
    maximum: u64,
) -> Result<PresetDocument, PresetCatalogError> {
    #[cfg(target_os = "linux")]
    let (mut file, advertised) = {
        let fd = fs::open(
            path,
            OFlags::RDONLY | OFlags::NONBLOCK | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::empty(),
        )
        .map_err(|_| PresetCatalogError::FileOpen)?;
        let stat = fs::fstat(&fd).map_err(|_| PresetCatalogError::FileRead)?;
        if !FileType::from_raw_mode(stat.st_mode).is_file() {
            return Err(PresetCatalogError::NotRegularFile);
        }
        let advertised =
            u64::try_from(stat.st_size).map_err(|_| PresetCatalogError::FileTooLarge {
                requested: u64::MAX,
                maximum,
            })?;
        (File::from(fd), advertised)
    };

    #[cfg(not(target_os = "linux"))]
    let (mut file, advertised) = {
        // Omalux targets Omarchy/Linux. Other platforms fail closed until
        // an equivalent atomic NOFOLLOW open is implemented.
        let _ = path;
        return Err(PresetCatalogError::NoFollowUnsupported);
    };

    if advertised > maximum {
        return Err(PresetCatalogError::FileTooLarge {
            requested: advertised,
            maximum,
        });
    }
    let initial = usize::try_from(advertised).map_err(|_| PresetCatalogError::FileTooLarge {
        requested: advertised,
        maximum,
    })?;
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(initial)
        .map_err(|_| PresetCatalogError::Allocation)?;
    let mut chunk = [0_u8; 16 * 1024];
    loop {
        let count = file
            .read(&mut chunk)
            .map_err(|_| PresetCatalogError::FileRead)?;
        if count == 0 {
            break;
        }
        let next = u64::try_from(bytes.len())
            .ok()
            .and_then(|length| length.checked_add(count as u64))
            .ok_or(PresetCatalogError::FileTooLarge {
                requested: u64::MAX,
                maximum,
            })?;
        if next > maximum {
            return Err(PresetCatalogError::FileTooLarge {
                requested: next,
                maximum,
            });
        }
        bytes
            .try_reserve_exact(count)
            .map_err(|_| PresetCatalogError::Allocation)?;
        bytes.extend_from_slice(&chunk[..count]);
    }
    let json = std::str::from_utf8(&bytes).map_err(|_| PresetCatalogError::InvalidUtf8)?;
    PresetDocument::from_json(json).map_err(PresetCatalogError::Preset)
}

#[derive(Debug)]
pub enum PresetCatalogError {
    Preset(PresetError),
    DuplicateId(String),
    NonCanonical(String),
    FileOpen,
    FileRead,
    NotRegularFile,
    FileTooLarge {
        requested: u64,
        maximum: u64,
    },
    InvalidUtf8,
    Allocation,
    #[cfg(not(target_os = "linux"))]
    NoFollowUnsupported,
}

impl fmt::Display for PresetCatalogError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Preset(error) => write!(formatter, "invalid preset: {error}"),
            Self::DuplicateId(id) => write!(formatter, "duplicate preset id {id:?}"),
            Self::NonCanonical(id) => write!(formatter, "built-in preset {id:?} is not canonical"),
            Self::FileOpen => formatter.write_str("could not safely open preset file"),
            Self::FileRead => formatter.write_str("could not read preset file"),
            Self::NotRegularFile => formatter.write_str("preset source is not a regular file"),
            Self::FileTooLarge { requested, maximum } => {
                write!(
                    formatter,
                    "preset contains {requested} bytes; maximum is {maximum}"
                )
            }
            Self::InvalidUtf8 => formatter.write_str("preset is not UTF-8 JSON"),
            Self::Allocation => formatter.write_str("preset allocation failed"),
            #[cfg(not(target_os = "linux"))]
            Self::NoFollowUnsupported => {
                formatter.write_str("safe preset file opening is unsupported on this platform")
            }
        }
    }
}

impl std::error::Error for PresetCatalogError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Preset(error) => Some(error),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn external_loader_rejects_growth_past_its_limit() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("preset.json");
        let mut file = File::create(&path).unwrap();
        file.write_all(b"123456789").unwrap();
        assert!(matches!(
            load_preset_file_with_limit(&path, 8),
            Err(PresetCatalogError::FileTooLarge { .. })
        ));
    }

    #[test]
    fn built_in_format_is_exact_canonical_json_plus_one_lf() {
        let canonical = PresetDocument::new("neutral", "Neutral", Default::default())
            .to_canonical_json()
            .unwrap();
        assert!(parse_builtin_json(&format!("{canonical}\n")).is_ok());
        for noncanonical in [
            canonical.clone(),
            format!("{canonical} \n"),
            format!("{canonical}\r\n"),
            format!("{canonical}\n\n"),
            format!(" {canonical}\n"),
        ] {
            assert!(matches!(
                parse_builtin_json(&noncanonical),
                Err(PresetCatalogError::NonCanonical(id)) if id == "neutral"
            ));
        }
    }
}
