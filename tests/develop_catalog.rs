use grainroom::develop::{
    CpuImage, DevelopPipeline, DevelopRenderContext, DevelopSettings, DevelopWorkingSetProfile,
    PresetCatalog, PresetCatalogError, PresetDocument, RgbaPixel, estimate_develop_working_set,
    load_preset_file,
};
use grainroom::io::ResourceLimits;
use std::{fs, io::Write};

#[test]
fn built_in_catalog_is_canonical_complete_sorted_and_searchable() {
    let catalog = PresetCatalog::built_in().unwrap();
    assert_eq!(catalog.documents().len(), 28);
    let ids = catalog
        .documents()
        .iter()
        .map(|document| document.id.as_str())
        .collect::<Vec<_>>();
    assert!(ids.windows(2).all(|pair| pair[0] < pair[1]));
    assert_eq!(ids.first(), Some(&"community-amber-grain"));
    assert_eq!(ids.last(), Some(&"series-cedar-fade"));
    assert_eq!(
        ids.iter().filter(|id| id.starts_with("personal-")).count(),
        13
    );
    assert_eq!(
        ids.iter().filter(|id| id.starts_with("community-")).count(),
        9
    );
    assert_eq!(ids.iter().filter(|id| id.starts_with("series-")).count(), 5);
    for document in catalog.documents() {
        let canonical = document.to_canonical_json().unwrap();
        assert_eq!(PresetDocument::from_json(&canonical).unwrap(), *document);
        let estimate =
            estimate_develop_working_set(64, 48, &document.settings, &ResourceLimits::default())
                .unwrap_or_else(|error| {
                    panic!("built-in {} has no bounded profile: {error}", document.id)
                });
        assert_eq!(estimate.output_width, 64);
        assert_eq!(estimate.output_height, 48);
        let pixels = (0..64 * 48)
            .map(|index| {
                let x = (index % 64) as f32 / 63.0;
                let y = (index / 64) as f32 / 47.0;
                RgbaPixel::new(x, y, (x + y) * 0.5, 1.0).unwrap()
            })
            .collect();
        let mut image = CpuImage::new(64, 48, pixels).unwrap();
        DevelopPipeline
            .process_bounded_with_context(
                &mut image,
                &document.settings,
                Some(&DevelopRenderContext::from_source_digest([0x73; 32])),
                &ResourceLimits::default(),
            )
            .unwrap_or_else(|error| panic!("built-in {} cannot render: {error}", document.id));
    }
    let neutral = catalog.get("neutral").unwrap();
    assert_eq!(neutral.name, "Neutral");
    assert!(neutral.settings.is_neutral());
    assert_eq!(
        format!("{}\n", neutral.to_canonical_json().unwrap()),
        include_str!("../presets/builtin/neutral.json")
    );
    let estimate = estimate_develop_working_set(
        3,
        2,
        &neutral.settings,
        &ResourceLimits::default().with_max_working_bytes(192),
    )
    .unwrap();
    assert_eq!(estimate.profile, DevelopWorkingSetProfile::PointwiseV1);
    assert_eq!(estimate.peak_bytes, 192);
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
    use std::os::unix::{fs::symlink, net::UnixListener};

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

    let fifo = directory.path().join("preset.fifo");
    rustix::fs::mkfifoat(
        rustix::fs::CWD,
        &fifo,
        rustix::fs::Mode::RUSR | rustix::fs::Mode::WUSR,
    )
    .unwrap();
    assert!(matches!(
        load_preset_file(&fifo),
        Err(PresetCatalogError::NotRegularFile)
    ));

    let socket = directory.path().join("preset.socket");
    let _listener = UnixListener::bind(&socket).unwrap();
    assert!(matches!(
        load_preset_file(&socket),
        Err(PresetCatalogError::FileOpen | PresetCatalogError::NotRegularFile)
    ));
    assert!(matches!(
        load_preset_file(directory.path()),
        Err(PresetCatalogError::NotRegularFile)
    ));
}
