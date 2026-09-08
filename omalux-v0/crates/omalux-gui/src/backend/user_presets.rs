use super::develop::{built_in_catalog_json, develop_preview};
use cxx_qt_lib::{QString, QUrl};
use omalux::{
    develop::DevelopSettings,
    job::CancellationToken,
    preset::{PresetDocument, load_preset_file},
};
use serde_json::json;
use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
};

type Result<T> = std::result::Result<T, String>;

pub(super) fn root() -> Result<PathBuf> {
    let base = std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .filter(|path| path.is_absolute())
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".local/share")))
        .ok_or("No user data directory")?;
    Ok(base.join("omalux/presets"))
}

fn directory(root: &Path, id: &str) -> Result<PathBuf> {
    if !id.starts_with("user-") || !id.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-') {
        return Err("Invalid user preset ID".into());
    }
    let path = root.join(id);
    if path.exists()
        && fs::symlink_metadata(&path)
            .map_err(|e| e.to_string())?
            .file_type()
            .is_symlink()
    {
        return Err("Preset directory must not be a symlink".into());
    }
    Ok(path)
}

pub(super) fn load(root: &Path, id: &str) -> Result<PresetDocument> {
    let document =
        load_preset_file(&directory(root, id)?.join("preset.json")).map_err(|e| e.to_string())?;
    if document.id != id {
        return Err("Preset ID does not match its directory".into());
    }
    Ok(document)
}

fn documents(root: &Path) -> Vec<PresetDocument> {
    let Ok(entries) = fs::read_dir(root) else {
        return vec![];
    };
    let mut documents: Vec<_> = entries
        .flatten()
        .filter_map(|entry| {
            let id = entry.file_name().to_string_lossy().into_owned();
            load(root, &id).ok()
        })
        .collect();
    documents.sort_by_key(|document| document.name.to_lowercase());
    documents
}

pub(super) fn catalog() -> Result<String> {
    let mut catalog: serde_json::Value =
        serde_json::from_str(&built_in_catalog_json()?).map_err(|e| e.to_string())?;
    if let Ok(root) = root() {
        for document in documents(&root) {
            let path = root.join(&document.id).join("thumbnail.jpg");
            let url = QUrl::from_local_file(&QString::from(path.to_string_lossy().as_ref()))
                .to_string()
                .to_string();
            let revision = fs::metadata(&path)
                .and_then(|meta| meta.modified())
                .ok()
                .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
                .map_or(0, |duration| duration.as_nanos());
            catalog["presets"].as_array_mut().unwrap().push(json!({
                "id": document.id, "name": document.name, "group": "my-presets",
                "previewUrl": format!("{url}?v={revision}"), "user": true
            }));
        }
    }
    serde_json::to_string(&catalog).map_err(|e| e.to_string())
}

fn look_settings(mut settings: DevelopSettings) -> DevelopSettings {
    settings.geometry = Default::default();
    settings.radial_masks = Default::default();
    settings
}

fn write_file(path: &Path, bytes: &[u8]) -> Result<()> {
    let mut file = fs::File::create(path).map_err(|e| e.to_string())?;
    file.write_all(bytes)
        .and_then(|()| file.sync_all())
        .map_err(|e| e.to_string())
}

// Publish JSON and thumbnail together. A failed render never touches the old preset.
fn publish(root: &Path, document: &PresetDocument, thumbnail: &[u8], replace: bool) -> Result<()> {
    fs::create_dir_all(root).map_err(|e| e.to_string())?;
    let stage = tempfile::Builder::new()
        .prefix(".saving-")
        .tempdir_in(root)
        .map_err(|e| e.to_string())?;
    let json = document.to_canonical_json().map_err(|e| e.to_string())? + "\n";
    write_file(&stage.path().join("preset.json"), json.as_bytes())?;
    write_file(&stage.path().join("thumbnail.jpg"), thumbnail)?;
    let destination = directory(root, &document.id)?;
    let flags = if replace {
        rustix::fs::RenameFlags::EXCHANGE
    } else {
        rustix::fs::RenameFlags::NOREPLACE
    };
    rustix::fs::renameat_with(
        rustix::fs::CWD,
        stage.path(),
        rustix::fs::CWD,
        &destination,
        flags,
    )
    .map_err(|e| e.to_string())?;
    fs::File::open(root)
        .and_then(|file| file.sync_all())
        .map_err(|e| e.to_string())?;
    Ok(())
}

pub(super) fn save(
    root: &Path,
    name: &str,
    mode: &str,
    settings: DevelopSettings,
) -> Result<PresetDocument> {
    let mut name = name.trim().to_owned();
    if name.is_empty() || name.len() > 160 {
        return Err("Enter a name of 1–160 bytes".into());
    }
    let existing = documents(root);
    let same_name = |name: &str| {
        existing
            .iter()
            .find(|doc| doc.name.to_lowercase() == name.to_lowercase())
    };
    let replace = mode == "replace";
    if mode == "copy" {
        let base = name.clone();
        let mut number = 2;
        while same_name(&name).is_some() {
            name = format!("{base} ({number})");
            number += 1;
        }
    } else if !replace && same_name(&name).is_some() {
        return Err("Name already exists. Choose Replace or Save copy.".into());
    }
    fs::create_dir_all(root).map_err(|e| e.to_string())?;
    let existing_id = if replace {
        same_name(&name).map(|doc| doc.id.clone())
    } else {
        None
    };
    let token = tempfile::Builder::new()
        .prefix("user-")
        .tempdir()
        .map_err(|e| e.to_string())?;
    let id = existing_id.clone().unwrap_or_else(|| {
        token
            .path()
            .file_name()
            .unwrap()
            .to_string_lossy()
            .into_owned()
    });
    let document = PresetDocument::new(id, name, look_settings(settings));
    document.validate().map_err(|e| e.to_string())?;

    let data = root.parent().ok_or("Invalid data directory")?;
    let reference = data.join("reference.jpg");
    if !reference.exists() {
        let mut file = tempfile::NamedTempFile::new_in(data).map_err(|e| e.to_string())?;
        file.write_all(include_bytes!("../../../../reference pictures/main.jpg"))
            .map_err(|e| e.to_string())?;
        file.as_file().sync_all().map_err(|e| e.to_string())?;
        match file.persist_noclobber(&reference) {
            Ok(_) => (),
            Err(error) if error.error.kind() == std::io::ErrorKind::AlreadyExists => (),
            Err(error) => return Err(error.to_string()),
        }
    }
    let preview = develop_preview(
        &reference,
        document.settings.clone(),
        false,
        &CancellationToken::new(),
    )
    .map_err(|e| e.to_string())?;
    let image = image::open(preview.path())
        .map_err(|e| e.to_string())?
        .thumbnail(288, 192);
    let mut thumbnail = Vec::new();
    image::codecs::jpeg::JpegEncoder::new_with_quality(&mut thumbnail, 85)
        .encode_image(&image)
        .map_err(|e| e.to_string())?;
    publish(root, &document, &thumbnail, existing_id.is_some())?;
    Ok(document)
}

pub(super) fn rename(root: &Path, id: &str, name: &str) -> Result<()> {
    let mut document = load(root, id)?;
    let name = name.trim();
    if documents(root)
        .iter()
        .any(|doc| doc.id != id && doc.name.to_lowercase() == name.to_lowercase())
    {
        return Err("A preset with that name already exists".into());
    }
    document.name = name.to_owned();
    document.validate().map_err(|e| e.to_string())?;
    let thumbnail =
        fs::read(directory(root, id)?.join("thumbnail.jpg")).map_err(|e| e.to_string())?;
    publish(root, &document, &thumbnail, true)
}

pub(super) fn delete(root: &Path, id: &str) -> Result<()> {
    load(root, id)?;
    fs::remove_dir_all(directory(root, id)?).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn personal_looks_exclude_image_specific_edits() {
        let mut settings = DevelopSettings::default();
        settings.geometry.quarter_turns_clockwise = 1;
        settings.basics.exposure_ev = 0.7;
        let look = look_settings(settings);
        assert_eq!(look.geometry, Default::default());
        assert_eq!(look.basics.exposure_ev, 0.7);
    }

    #[test]
    fn saved_look_contains_a_real_thumbnail_and_survives_reload() {
        let data = tempfile::tempdir().unwrap();
        let root = data.path().join("omalux/presets");
        let mut settings = DevelopSettings::default();
        settings.geometry.quarter_turns_clockwise = 1;
        settings.basics.exposure_ev = 0.5;
        let document = save(&root, "Beach test", "new", settings).unwrap();
        let loaded = load(&root, &document.id).unwrap();
        assert_eq!(loaded.settings.geometry, Default::default());
        assert_eq!(loaded.settings.basics.exposure_ev, 0.5);
        let thumbnail = image::open(root.join(&document.id).join("thumbnail.jpg")).unwrap();
        assert!(thumbnail.width() <= 288 && thumbnail.height() <= 192);
        assert!(data.path().join("omalux/reference.jpg").is_file());
        assert!(save(&root, "Beach test", "new", DevelopSettings::default()).is_err());
        assert_eq!(documents(&root).len(), 1);
    }

    #[test]
    fn publication_replaces_a_complete_pair_and_rejects_traversal() {
        let root = tempfile::tempdir().unwrap();
        let mut document = PresetDocument::new("user-test", "Test", DevelopSettings::default());
        publish(root.path(), &document, b"first", false).unwrap();
        document.settings.basics.exposure_ev = 1.0;
        assert!(publish(root.path(), &document, b"second", false).is_err());
        assert_eq!(
            load(root.path(), "user-test")
                .unwrap()
                .settings
                .basics
                .exposure_ev,
            0.0
        );
        publish(root.path(), &document, b"second", true).unwrap();
        assert_eq!(
            load(root.path(), "user-test")
                .unwrap()
                .settings
                .basics
                .exposure_ev,
            1.0
        );
        assert_eq!(
            fs::read(root.path().join("user-test/thumbnail.jpg")).unwrap(),
            b"second"
        );
        assert!(load(root.path(), "../user-test").is_err());
        rename(root.path(), "user-test", "Renamed").unwrap();
        assert_eq!(load(root.path(), "user-test").unwrap().name, "Renamed");
        delete(root.path(), "user-test").unwrap();
        assert!(documents(root.path()).is_empty());
    }
}
