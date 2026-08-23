use grainroom::develop::{
    CpuImage, CurvePoint, DevelopPipeline, DevelopSettings, LocalAdjustments, ParameterOverride,
    PresetDocument, PresetError, RadialMask, RgbaPixel, ToneCurve, apply_parameter_overrides,
    parameter_registry,
};

const V1_NEUTRAL: &str = include_str!("fixtures/preset-v1-neutral.json");

fn image(values: &[f32]) -> CpuImage {
    CpuImage::new(
        values.len() as u32,
        1,
        values
            .iter()
            .map(|&value| RgbaPixel::new(value, value, value, 0.37).unwrap())
            .collect(),
    )
    .unwrap()
}

fn render(values: &[f32], settings: &DevelopSettings) -> CpuImage {
    let mut output = image(values);
    DevelopPipeline.process(&mut output, settings).unwrap();
    output
}

#[test]
fn exposure_ev_has_exact_stop_goldens_and_precedes_contrast() {
    for (ev, expected) in [(-4.0, 0.01125), (-2.0, 0.045), (2.0, 0.72), (4.0, 2.88)] {
        let mut settings = DevelopSettings::default();
        settings.basics.exposure_ev = ev;
        let pixel = render(&[0.18], &settings).pixels()[0];
        assert!((pixel.red() - expected).abs() < 2.0e-6, "ev={ev}");
        assert_eq!(pixel.alpha(), 0.37);
    }

    let mut settings = DevelopSettings::default();
    settings.basics.exposure_ev = 1.0;
    settings.basics.contrast = 100.0;
    let pixel = render(&[0.18], &settings).pixels()[0];
    // Exposure first yields 0.36, then the contrast slope of two yields 0.72.
    assert!((pixel.red() - 0.72).abs() < 2.0e-6);
}

#[test]
fn exposure_overflow_is_transactional() {
    let original = image(&[f32::MAX]);
    let mut candidate = original.clone();
    let mut settings = DevelopSettings::default();
    settings.basics.exposure_ev = 4.0;
    assert!(DevelopPipeline.process(&mut candidate, &settings).is_err());
    assert_eq!(candidate, original);
}

fn seven_point_curve() -> ToneCurve {
    ToneCurve {
        points: [-0.6_f32, -0.2, 0.0, 0.18, 0.7, 1.0, 1.6]
            .into_iter()
            .map(|x| CurvePoint {
                x,
                y: 0.5 * x + 0.2,
            })
            .collect(),
    }
}

#[test]
fn seven_point_extended_curves_hit_every_node_on_master_and_rgb() {
    let curve = seven_point_curve();
    let inputs: Vec<f32> = curve.points.iter().map(|point| point.x).collect();
    let expected: Vec<f32> = curve.points.iter().map(|point| point.y).collect();
    for channel in 0..4 {
        let mut settings = DevelopSettings::default();
        match channel {
            0 => settings.tone_curves.master = curve.clone(),
            1 => settings.tone_curves.red = curve.clone(),
            2 => settings.tone_curves.green = curve.clone(),
            _ => settings.tone_curves.blue = curve.clone(),
        }
        settings.validate().unwrap();
        let output = render(&inputs, &settings);
        for (index, pixel) in output.pixels().iter().enumerate() {
            let actual = match channel {
                0 | 1 => pixel.red(),
                2 => pixel.green(),
                _ => pixel.blue(),
            };
            assert!(
                (actual - expected[index]).abs() < 3.0e-6,
                "channel={channel} node={index}"
            );
        }
    }
}

#[test]
fn extended_curve_interpolates_monotonically_and_extrapolates_signed_hdr() {
    let mut settings = DevelopSettings::default();
    settings.tone_curves.red = seven_point_curve();
    let inputs: Vec<f32> = (-100..=200).map(|value| value as f32 / 100.0).collect();
    let output = render(&inputs, &settings);
    let values: Vec<f32> = output.pixels().iter().map(RgbaPixel::red).collect();
    assert!(values.windows(2).all(|pair| pair[0] <= pair[1]));
    assert!((values[0] - -0.3).abs() < 3.0e-6);
    assert!((values[values.len() - 1] - 1.2).abs() < 3.0e-6);
}

#[test]
fn nonlinear_seven_point_pchip_has_extended_segment_goldens() {
    let mut settings = DevelopSettings::default();
    settings.tone_curves.red = ToneCurve {
        points: [
            (-0.6, -0.5),
            (-0.3, -0.1),
            (0.0, 0.05),
            (0.2, 0.35),
            (0.6, 0.5),
            (1.0, 1.1),
            (1.6, 1.8),
        ]
        .into_iter()
        .map(|(x, y)| CurvePoint { x, y })
        .collect(),
    };
    let inputs = [-0.9, -0.45, -0.15, 0.1, 0.4, 0.8, 1.3, 2.0];
    let expected = [
        -1.025,
        -0.261_647_73,
        -0.026_822_1,
        0.203_325_12,
        0.427_142_86,
        0.763_823_5,
        1.476_764_7,
        2.186_666_7,
    ];
    let output = render(&inputs, &settings);
    for (index, pixel) in output.pixels().iter().enumerate() {
        assert!(
            (pixel.red() - expected[index]).abs() < 2.0e-5,
            "sample={index}: {} != {}",
            pixel.red(),
            expected[index]
        );
    }
    // The first interior sample is deliberately not the straight secant
    // midpoint, proving the cubic path is exercised in the negative domain.
    assert!((output.pixels()[1].red() - -0.3).abs() > 0.03);
}

#[test]
fn preset_v1_migrates_explicitly_and_v2_is_strict() {
    let migrated = PresetDocument::from_json(V1_NEUTRAL).unwrap();
    assert_eq!(migrated.schema_version, 2);
    assert_eq!(migrated.settings.basics.exposure_ev, 0.0);
    assert!(
        migrated
            .to_canonical_json()
            .unwrap()
            .contains("\"exposure_ev\":0.0")
    );

    let illegal_v1 = V1_NEUTRAL.replacen(
        "\"brightness\":0.0",
        "\"exposure_ev\":1.0,\"brightness\":0.0",
        1,
    );
    assert!(matches!(
        PresetDocument::from_json(&illegal_v1),
        Err(PresetError::FieldNotAvailable { version: 1, .. })
    ));
    let extended_v1 = V1_NEUTRAL.replacen("\"x\":0.0,\"y\":0.0", "\"x\":-0.6,\"y\":0.0", 1);
    assert!(matches!(
        PresetDocument::from_json(&extended_v1),
        Err(PresetError::FieldNotAvailable {
            version: 1,
            path: "settings.tone_curves.master"
        })
    ));
    assert!(serde_json::from_str::<PresetDocument>(&extended_v1).is_err());

    let missing_v2 = migrated
        .to_canonical_json()
        .unwrap()
        .replacen("\"exposure_ev\":0.0,", "", 1);
    assert!(matches!(
        PresetDocument::from_json(&missing_v2),
        Err(PresetError::MissingRequiredField(
            "settings.basics.exposure_ev"
        ))
    ));

    let direct_v1: PresetDocument = serde_json::from_str(V1_NEUTRAL).unwrap();
    assert_eq!(direct_v1, migrated);
    assert!(serde_json::from_str::<PresetDocument>(&missing_v2).is_err());
    let direct_v2: PresetDocument =
        serde_json::from_str(&migrated.to_canonical_json().unwrap()).unwrap();
    assert_eq!(direct_v2, migrated);
}

fn preset_with_local_exposure(exposure_ev: f32) -> PresetDocument {
    let mut settings = DevelopSettings::default();
    settings.radial_masks.masks.push(RadialMask {
        id: "schema-mask".to_owned(),
        enabled: true,
        center_x: 0.5,
        center_y: 0.5,
        radius_x: 0.25,
        radius_y: 0.25,
        rotation_degrees: 0.0,
        feather: 0.5,
        opacity: 1.0,
        invert: false,
        adjustments: LocalAdjustments {
            exposure_ev,
            ..LocalAdjustments::default()
        },
    });
    PresetDocument::new("local-schema", "Local schema", settings)
}

#[test]
fn local_exposure_is_required_in_v2_and_migrated_only_from_v1() {
    let v2 = preset_with_local_exposure(0.0).to_canonical_json().unwrap();
    let missing_v2 = v2.replacen(
        "\"adjustments\":{\"exposure_ev\":0.0,",
        "\"adjustments\":{",
        1,
    );
    assert!(matches!(
        PresetDocument::from_json(&missing_v2),
        Err(PresetError::MissingRequiredField(
            "settings.radial_masks.masks[].adjustments.exposure_ev"
        ))
    ));
    assert!(serde_json::from_str::<PresetDocument>(&missing_v2).is_err());

    let v1 = missing_v2
        .replacen("\"schema_version\":2", "\"schema_version\":1", 1)
        .replacen("\"exposure_ev\":0.0,", "", 1);
    let migrated = PresetDocument::from_json(&v1).unwrap();
    assert_eq!(
        migrated.settings.radial_masks.masks[0]
            .adjustments
            .exposure_ev,
        0.0
    );
    let direct: PresetDocument = serde_json::from_str(&v1).unwrap();
    assert_eq!(direct, migrated);

    let illegal_v1 = v1.replacen(
        "\"adjustments\":{",
        "\"adjustments\":{\"exposure_ev\":1.0,",
        1,
    );
    assert!(matches!(
        PresetDocument::from_json(&illegal_v1),
        Err(PresetError::FieldNotAvailable {
            version: 1,
            path: "settings.radial_masks.masks[].adjustments.exposure_ev"
        })
    ));
    assert!(serde_json::from_str::<PresetDocument>(&illegal_v1).is_err());
}

#[test]
fn registry_and_override_expose_typed_stops() {
    let registry = parameter_registry();
    let definition = registry
        .iter()
        .find(|definition| definition.id == "basics.exposure_ev")
        .unwrap();
    assert_eq!(
        (definition.minimum, definition.maximum, definition.step),
        (-4.0, 4.0, 0.1)
    );
    let curve = registry
        .iter()
        .find(|definition| definition.id == "tone_curves.master")
        .unwrap();
    assert_eq!((curve.minimum, curve.maximum), (-4.0, 4.0));
    let toggle = registry
        .iter()
        .find(|definition| definition.id == "geometry.flip_horizontal")
        .unwrap();
    assert_eq!((toggle.minimum, toggle.maximum), (0.0, 1.0));
    let settings = apply_parameter_overrides(
        &DevelopSettings::default(),
        &[ParameterOverride::scalar("basics.exposure_ev", 1.5)],
    )
    .unwrap();
    assert_eq!(settings.basics.exposure_ev, 1.5);
    let local = registry
        .iter()
        .find(|definition| definition.id == "radial_masks[].adjustments.exposure_ev")
        .unwrap();
    assert_eq!((local.minimum, local.maximum, local.step), (-4.0, 4.0, 0.1));
}
