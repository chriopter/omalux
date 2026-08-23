use grainroom::develop::settings::{CurvePoint, RadialMask, ToneCurve};
use grainroom::develop::{
    CANONICAL_STAGE_ORDER, CpuImage, DevelopPipeline, DevelopSettings, DevelopStage, ImageError,
    LocalAdjustments, NeutralRepresentation, ParameterKind, PipelineError, PixelChannel,
    PixelError, PresetDocument, PresetError, RgbaPixel, parameter_registry,
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
    let unknown_version = r#"{
        "schema": "io.omacom.grainroom.preset",
        "schema_version": 99,
        "future_payload": { "completely": ["different"] }
    }"#;
    assert!(matches!(
        PresetDocument::from_json(unknown_version),
        Err(PresetError::UnsupportedVersion(99))
    ));

    let unknown_schema = NEUTRAL_PRESET.replacen(
        "io.omacom.grainroom.preset",
        "example.invalid.future-preset",
        1,
    );
    assert!(matches!(
        PresetDocument::from_json(&unknown_schema),
        Err(PresetError::UnsupportedSchema(_))
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
        "\"amount\":0.0,",
        "\"amount\":0.0,\"unknown_grain_field\":0.0,",
        1,
    );
    assert!(matches!(
        PresetDocument::from_json(&unknown_nested),
        Err(PresetError::Json(_))
    ));

    let unknown_curve_point = NEUTRAL_PRESET.replacen(
        "{\"x\":0.0,\"y\":0.0}",
        "{\"x\":0.0,\"y\":0.0,\"tension\":0.5}",
        1,
    );
    assert!(matches!(
        PresetDocument::from_json(&unknown_curve_point),
        Err(PresetError::Json(_))
    ));
}

#[test]
fn parameter_registry_is_stable_unique_and_well_ranged() {
    let registry = parameter_registry();
    assert_eq!(registry.len(), 88);
    let unique: HashSet<_> = registry.iter().map(|definition| &definition.id).collect();
    assert_eq!(unique.len(), registry.len());
    for definition in &registry {
        assert!(!definition.id.is_empty());
        assert!(definition.minimum <= definition.neutral);
        assert!(definition.neutral <= definition.maximum);
        assert!(definition.step > 0.0);
    }

    let ids = registry
        .iter()
        .map(|definition| definition.id.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    assert_eq!(ids, expected_parameter_ids());

    let crop_presence = registry
        .iter()
        .find(|definition| definition.id == "geometry.crop.enabled")
        .unwrap();
    assert_eq!(crop_presence.kind, ParameterKind::Presence);
    assert_eq!(
        crop_presence.neutral_representation,
        NeutralRepresentation::Absent
    );

    let masks = registry
        .iter()
        .find(|definition| definition.id == "radial_masks")
        .unwrap();
    assert_eq!(masks.kind, ParameterKind::Collection);
    assert_eq!(
        masks.neutral_representation,
        NeutralRepresentation::EmptyCollection
    );
    assert_eq!(masks.maximum, 64.0);

    let mask_id = registry
        .iter()
        .find(|definition| definition.id == "radial_masks[].id")
        .unwrap();
    assert_eq!(mask_id.kind, ParameterKind::Identifier);
    assert_eq!(
        mask_id.neutral_representation,
        NeutralRepresentation::NotApplicable
    );

    for curve in registry
        .iter()
        .filter(|definition| definition.kind == ParameterKind::Curve)
    {
        assert_eq!(
            curve.neutral_representation,
            NeutralRepresentation::IdentityCurve
        );
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
        RgbaPixel::new(-0.25, 0.25, 4.0, 1.0).unwrap(),
        RgbaPixel::new(0.7, 0.1, 0.3, 0.5).unwrap(),
    ];
    let mut image = CpuImage::new(2, 1, pixels).unwrap();
    let original = image.clone();
    DevelopPipeline
        .process(&mut image, &DevelopSettings::default())
        .unwrap();
    assert_eq!(image, original);
}

#[test]
fn capability_preflight_is_complete_and_every_error_is_atomic() {
    for (stage, settings) in unsupported_settings_by_stage() {
        let mut image = valid_image();
        let original = image.clone();
        assert_eq!(
            DevelopPipeline.preflight(&settings),
            Err(PipelineError::StageNotImplemented(stage))
        );
        assert_eq!(
            DevelopPipeline.process(&mut image, &settings),
            Err(PipelineError::StageNotImplemented(stage))
        );
        assert_eq!(image, original, "{stage:?} failure mutated input");
    }

    let mut image = valid_image();
    let original = image.clone();
    let mut invalid = DevelopSettings::default();
    invalid.basics.brightness = f32::NAN;
    assert!(matches!(
        DevelopPipeline.process(&mut image, &invalid),
        Err(PipelineError::InvalidSettings(_))
    ));
    assert_eq!(image, original, "validation failure mutated input");
}

#[test]
fn implemented_stages_preflight_and_process_non_neutral_settings() {
    let mut cases = Vec::new();

    let mut settings = DevelopSettings::default();
    settings.geometry.quarter_turns_clockwise = 1;
    settings.geometry.perspective_horizontal = 10.0;
    settings.geometry.perspective_vertical = -5.0;
    settings.geometry.crop = Some(grainroom::develop::CropRect {
        x: 0.0,
        y: 0.0,
        width: 1.0,
        height: 0.5,
    });
    cases.push((DevelopStage::Geometry, settings));

    let mut settings = DevelopSettings::default();
    settings.basics.clarity = 10.0;
    cases.push((DevelopStage::Basics, settings));

    let mut settings = DevelopSettings::default();
    settings.tone_curves.master.points = vec![
        CurvePoint { x: 0.0, y: 0.0 },
        CurvePoint { x: 0.5, y: 0.6 },
        CurvePoint { x: 1.0, y: 1.0 },
    ];
    cases.push((DevelopStage::ToneCurves, settings));

    let mut settings = DevelopSettings::default();
    settings.color_mixer.orange.saturation = 10.0;
    cases.push((DevelopStage::ColorMixer, settings));

    let mut settings = DevelopSettings::default();
    settings.color_grading.midtones.saturation = 10.0;
    cases.push((DevelopStage::ColorGrading, settings));

    let mut settings = DevelopSettings::default();
    let mut mask = radial_mask("supported");
    mask.adjustments.brightness = 10.0;
    settings.radial_masks.masks.push(mask);
    cases.push((DevelopStage::RadialMasks, settings));

    let mut settings = DevelopSettings::default();
    settings.effects.bloom = 10.0;
    cases.push((DevelopStage::Effects, settings));

    for (stage, settings) in cases {
        assert_eq!(DevelopPipeline.preflight(&settings), Ok(()), "{stage:?}");
        let mut image = valid_image();
        assert_eq!(
            DevelopPipeline.process(&mut image, &settings),
            Ok(()),
            "{stage:?}"
        );
    }
}

#[test]
fn pixel_contract_accepts_unbounded_finite_rgb_and_rejects_invalid_values() {
    let pixel = RgbaPixel::new(-12.0, 0.18, 32.0, 0.25).unwrap();
    assert_eq!(pixel.red(), -12.0);
    assert_eq!(pixel.green(), 0.18);
    assert_eq!(pixel.blue(), 32.0);
    assert_eq!(pixel.alpha(), 0.25);

    assert_eq!(
        RgbaPixel::new(f32::NAN, 0.0, 0.0, 1.0),
        Err(PixelError::NonFinite(PixelChannel::Red))
    );
    assert_eq!(
        RgbaPixel::new(0.0, f32::INFINITY, 0.0, 1.0),
        Err(PixelError::NonFinite(PixelChannel::Green))
    );
    assert_eq!(
        RgbaPixel::new(0.0, 0.0, 0.0, -0.01),
        Err(PixelError::AlphaOutOfRange)
    );
    assert_eq!(
        CpuImage::new(u32::MAX, u32::MAX, Vec::new()),
        Err(ImageError::DimensionOverflow {
            width: u32::MAX,
            height: u32::MAX
        })
    );
}

#[test]
fn grading_modifiers_are_dormant_without_active_wheels() {
    let mut settings = DevelopSettings::default();
    settings.color_grading.balance = 100.0;
    settings.color_grading.blending = 100.0;
    assert!(settings.color_grading.is_neutral());
    assert!(settings.is_neutral());

    settings.color_grading.shadows.saturation = 1.0;
    assert!(!settings.color_grading.is_neutral());
}

#[test]
fn canonical_json_normalizes_negative_zero_and_equivalent_hues() {
    let mut document = PresetDocument::from_json(NEUTRAL_PRESET).unwrap();
    document.settings.basics.brightness = -0.0;
    document.settings.color_grading.shadows.hue_degrees = 360.0;
    document.settings.color_mixer.red.hue_shift_degrees = 180.0;
    let mut mask = radial_mask("canonical");
    mask.enabled = false;
    mask.rotation_degrees = 180.0;
    mask.adjustments.tint = -0.0;
    document.settings.radial_masks.masks.push(mask);

    let canonical = document.to_canonical_json().unwrap();
    assert!(!canonical.contains("-0.0"));
    let reparsed = PresetDocument::from_json(&canonical).unwrap();
    assert_eq!(reparsed.settings.color_grading.shadows.hue_degrees, 0.0);
    assert_eq!(reparsed.settings.color_mixer.red.hue_shift_degrees, -180.0);
    assert_eq!(
        reparsed.settings.radial_masks.masks[0].rotation_degrees,
        -180.0
    );
    assert_eq!(reparsed.to_canonical_json().unwrap(), canonical);
}

#[test]
fn validation_rejects_nan_infinity_and_non_json_numbers() {
    let mut settings = DevelopSettings::default();
    settings.basics.brightness = f32::NAN;
    assert_eq!(settings.validate().unwrap_err().path(), "basics.brightness");

    settings = DevelopSettings::default();
    settings.effects.grain.amount = f32::INFINITY;
    assert_eq!(
        settings.validate().unwrap_err().path(),
        "effects.grain.amount"
    );

    let invalid_json = NEUTRAL_PRESET.replacen("\"brightness\":0.0", "\"brightness\":NaN", 1);
    assert!(matches!(
        PresetDocument::from_json(&invalid_json),
        Err(PresetError::Json(_))
    ));

    let overflow_json = NEUTRAL_PRESET.replacen("\"brightness\":0.0", "\"brightness\":1e400", 1);
    assert!(matches!(
        PresetDocument::from_json(&overflow_json),
        Err(PresetError::Json(_))
    ));
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

fn valid_image() -> CpuImage {
    CpuImage::new(
        2,
        1,
        vec![
            RgbaPixel::new(-0.5, 0.25, 3.0, 1.0).unwrap(),
            RgbaPixel::new(0.4, 0.5, 0.6, 0.75).unwrap(),
        ],
    )
    .unwrap()
}

fn unsupported_settings_by_stage() -> Vec<(DevelopStage, DevelopSettings)> {
    let mut cases = Vec::new();

    let mut settings = DevelopSettings::default();
    let mut mask = radial_mask("active");
    mask.adjustments.sharpness = -1.0;
    settings.radial_masks.masks.push(mask);
    cases.push((DevelopStage::RadialMasks, settings));

    cases
}

fn expected_parameter_ids() -> &'static str {
    "geometry.quarter_turns_clockwise
geometry.straighten_degrees
geometry.perspective_horizontal
geometry.perspective_vertical
geometry.flip_horizontal
geometry.flip_vertical
geometry.crop.enabled
geometry.crop.x
geometry.crop.y
geometry.crop.width
geometry.crop.height
basics.exposure_ev
basics.brightness
basics.contrast
basics.clarity
basics.highlights
basics.shadows
basics.whites
basics.blacks
basics.saturation
basics.vibrance
basics.temperature
basics.tint
tone_curves.master
tone_curves.red
tone_curves.green
tone_curves.blue
color_mixer.red.hue_shift_degrees
color_mixer.red.saturation
color_mixer.red.luminance
color_mixer.orange.hue_shift_degrees
color_mixer.orange.saturation
color_mixer.orange.luminance
color_mixer.yellow.hue_shift_degrees
color_mixer.yellow.saturation
color_mixer.yellow.luminance
color_mixer.green.hue_shift_degrees
color_mixer.green.saturation
color_mixer.green.luminance
color_mixer.aqua.hue_shift_degrees
color_mixer.aqua.saturation
color_mixer.aqua.luminance
color_mixer.blue.hue_shift_degrees
color_mixer.blue.saturation
color_mixer.blue.luminance
color_mixer.purple.hue_shift_degrees
color_mixer.purple.saturation
color_mixer.purple.luminance
color_mixer.magenta.hue_shift_degrees
color_mixer.magenta.saturation
color_mixer.magenta.luminance
color_grading.shadows.hue_degrees
color_grading.shadows.saturation
color_grading.shadows.luminance
color_grading.midtones.hue_degrees
color_grading.midtones.saturation
color_grading.midtones.luminance
color_grading.highlights.hue_degrees
color_grading.highlights.saturation
color_grading.highlights.luminance
color_grading.blending
color_grading.balance
effects.bloom
effects.halation
effects.fade
effects.vignette
effects.sharpness
effects.grain.amount
effects.grain.size_iso
effects.grain.midtone_response
radial_masks
radial_masks[].id
radial_masks[].center_x
radial_masks[].center_y
radial_masks[].radius_x
radial_masks[].radius_y
radial_masks[].rotation_degrees
radial_masks[].feather
radial_masks[].opacity
radial_masks[].enabled
radial_masks[].invert
radial_masks[].adjustments.exposure_ev
radial_masks[].adjustments.brightness
radial_masks[].adjustments.contrast
radial_masks[].adjustments.saturation
radial_masks[].adjustments.temperature
radial_masks[].adjustments.tint
radial_masks[].adjustments.sharpness"
}
