use grainroom::develop::settings::{CurvePoint, RadialMask, ToneCurve};
use grainroom::develop::{
    CANONICAL_STAGE_ORDER, CpuImage, DevelopPipeline, DevelopSettings, DevelopStage,
    LocalAdjustments, PipelineError, PresetDocument, PresetError, RgbaPixel, parameter_registry,
};
use std::collections::HashSet;

const NEUTRAL_PRESET: &str = include_str!("fixtures/preset-v1-neutral.json");

#[test]
fn neutral_settings_are_valid_and_semantically_neutral() {
    let settings = DevelopSettings::default();
    assert_eq!(settings.validate(), Ok(()));
    assert!(settings.is_neutral());
}

#[test]
fn validation_covers_every_settings_family() {
    let mut settings = DevelopSettings::default();
    settings.geometry.quarter_turns_clockwise = 4;
    assert_eq!(
        settings.validate().unwrap_err().path(),
        "geometry.quarter_turns_clockwise"
    );

    settings = DevelopSettings::default();
    settings.basics.brightness = 101.0;
    assert_eq!(settings.validate().unwrap_err().path(), "basics.brightness");

    settings = DevelopSettings::default();
    settings.tone_curves.master = ToneCurve {
        points: vec![
            CurvePoint { x: 0.0, y: 0.0 },
            CurvePoint { x: 0.0, y: 0.5 },
            CurvePoint { x: 1.0, y: 1.0 },
        ],
    };
    assert_eq!(
        settings.validate().unwrap_err().path(),
        "tone_curves.master.points[1].x"
    );

    settings = DevelopSettings::default();
    settings.color_mixer.red.hue_shift_degrees = 181.0;
    assert_eq!(
        settings.validate().unwrap_err().path(),
        "color_mixer.red.hue_shift_degrees"
    );

    settings = DevelopSettings::default();
    settings.color_grading.highlights.hue_degrees = 361.0;
    assert_eq!(
        settings.validate().unwrap_err().path(),
        "color_grading.highlights.hue_degrees"
    );

    settings = DevelopSettings::default();
    settings.effects.grain.size_iso = 10.0;
    assert_eq!(
        settings.validate().unwrap_err().path(),
        "effects.grain.size_iso"
    );

    settings = DevelopSettings::default();
    settings.radial_masks.masks = vec![radial_mask("same"), radial_mask("same")];
    assert_eq!(
        settings.validate().unwrap_err().path(),
        "radial_masks.masks[1].id"
    );
}

#[test]
fn preset_v1_has_a_canonical_roundtrip() {
    let document = PresetDocument::from_json(NEUTRAL_PRESET).unwrap();
    assert!(document.settings.is_neutral());

    let canonical = document.to_canonical_json().unwrap();
    let reparsed = PresetDocument::from_json(&canonical).unwrap();
    assert_eq!(reparsed, document);
    assert_eq!(reparsed.to_canonical_json().unwrap(), canonical);
    assert!(canonical.contains("\"radial_masks\":{\"masks\":[]}"));
}

#[test]
fn preset_parser_rejects_unknown_versions_and_fields() {
    let unknown_version =
        NEUTRAL_PRESET.replacen("\"schema_version\":1", "\"schema_version\":99", 1);
    assert!(matches!(
        PresetDocument::from_json(&unknown_version),
        Err(PresetError::UnsupportedVersion(99))
    ));

    let unknown_root = NEUTRAL_PRESET.replacen(
        "\"name\":\"Neutral\",",
        "\"name\":\"Neutral\",\"unknown\":true,",
        1,
    );
    assert!(matches!(
        PresetDocument::from_json(&unknown_root),
        Err(PresetError::Json(_))
    ));

    let unknown_nested = NEUTRAL_PRESET.replacen(
        "\"brightness\":0.0,",
        "\"brightness\":0.0,\"exposure\":0.0,",
        1,
    );
    assert!(matches!(
        PresetDocument::from_json(&unknown_nested),
        Err(PresetError::Json(_))
    ));
}

#[test]
fn parameter_registry_is_stable_unique_and_well_ranged() {
    let registry = parameter_registry();
    assert_eq!(registry.len(), 83);
    let unique: HashSet<_> = registry.iter().map(|definition| &definition.id).collect();
    assert_eq!(unique.len(), registry.len());
    for definition in registry {
        assert!(!definition.id.is_empty());
        assert!(definition.minimum <= definition.neutral);
        assert!(definition.neutral <= definition.maximum);
        assert!(definition.step > 0.0);
    }
}

#[test]
fn pipeline_stage_order_is_explicit_and_stable() {
    assert_eq!(
        CANONICAL_STAGE_ORDER,
        [
            DevelopStage::Geometry,
            DevelopStage::Basics,
            DevelopStage::ToneCurves,
            DevelopStage::ColorMixer,
            DevelopStage::ColorGrading,
            DevelopStage::RadialMasks,
            DevelopStage::Effects,
        ]
    );
    assert_eq!(DevelopPipeline.stages(), &CANONICAL_STAGE_ORDER);
}

#[test]
fn neutral_cpu_pipeline_is_pixel_identical() {
    let pixels = vec![
        RgbaPixel::new(0.0, 0.25, 1.0, 1.0),
        RgbaPixel::new(0.7, 0.1, 0.3, 0.5),
    ];
    let mut image = CpuImage::new(2, 1, pixels).unwrap();
    let original = image.clone();
    DevelopPipeline
        .process(&mut image, &DevelopSettings::default())
        .unwrap();
    assert_eq!(image, original);
}

#[test]
fn non_neutral_unimplemented_stage_fails_loudly() {
    let mut image = CpuImage::new(1, 1, vec![RgbaPixel::new(0.5, 0.5, 0.5, 1.0)]).unwrap();
    let mut settings = DevelopSettings::default();
    settings.basics.contrast = 10.0;
    assert_eq!(
        DevelopPipeline.process(&mut image, &settings),
        Err(PipelineError::StageNotImplemented(DevelopStage::Basics))
    );
}

fn radial_mask(id: &str) -> RadialMask {
    RadialMask {
        id: id.to_owned(),
        enabled: true,
        center_x: 0.5,
        center_y: 0.5,
        radius_x: 0.25,
        radius_y: 0.25,
        rotation_degrees: 0.0,
        feather: 0.5,
        opacity: 1.0,
        invert: false,
        adjustments: LocalAdjustments::default(),
    }
}
