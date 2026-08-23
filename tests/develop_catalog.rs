use grainroom::develop::{
    DevelopSettings, PresetCatalog, PresetCatalogError, PresetDocument, load_preset_file,
};
use std::{fs, io::Write};

#[test]
fn built_in_catalog_is_canonical_neutral_and_searchable() {
    let catalog = PresetCatalog::built_in().unwrap();
    assert_eq!(catalog.documents().len(), 1);
    let neutral = catalog.get("neutral").unwrap();
    assert_eq!(neutral.name, "Neutral");
    assert!(neutral.settings.is_neutral());
    assert_eq!(
        neutral.to_canonical_json().unwrap(),
        include_str!("../presets/builtin/neutral.json").trim_end()
    );
    assert!(catalog.get("missing").is_none());
}

#[test]
fn catalog_sorts_canonicalizes_and_rejects_duplicate_ids() {
    let mut settings = DevelopSettings::default();
    settings.basics.contrast = -0.0;
    let last = PresetDocument::new("z-last", "Last", settings);
    let first = PresetDocument::new("a-first", "First", DevelopSettings::default());
    let catalog = PresetCatalog::from_documents(vec![last.clone(), first]).unwrap();
    assert_eq!(catalog.documents()[0].id, "a-first");
    assert_eq!(catalog.documents()[1].id, "z-last");
    assert_eq!(catalog.documents()[1].settings.basics.contrast.to_bits(), 0);
    assert!(matches!(
        PresetCatalog::from_documents(vec![last.clone(), last]),
        Err(PresetCatalogError::DuplicateId(id)) if id == "z-last"
    ));
}

#[test]
#[cfg(target_os = "linux")]
fn external_loader_is_bounded_nofollow_and_schema_validated() {
    use std::os::unix::fs::symlink;

    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("neutral.json");
    fs::write(&path, include_str!("../presets/builtin/neutral.json")).unwrap();
    assert_eq!(load_preset_file(&path).unwrap().id, "neutral");

    let link = directory.path().join("link.json");
    symlink(&path, &link).unwrap();
    assert!(matches!(
        load_preset_file(&link),
        Err(PresetCatalogError::FileOpen)
    ));

    let oversized = directory.path().join("oversized.json");
    let mut file = fs::File::create(&oversized).unwrap();
    file.write_all(&vec![b' '; 1024 * 1024 + 1]).unwrap();
    assert!(matches!(
        load_preset_file(&oversized),
        Err(PresetCatalogError::FileTooLarge { .. })
    ));

    let malformed = directory.path().join("malformed.json");
    fs::write(&malformed, b"{}").unwrap();
    assert!(matches!(
        load_preset_file(&malformed),
        Err(PresetCatalogError::Preset(_))
    ));
}
