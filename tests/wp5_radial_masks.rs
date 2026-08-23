use omalux::develop::{
    CpuImage, DevelopPipeline, DevelopSettings, DevelopStage, LocalAdjustments, PipelineError,
    RadialMask, RgbaPixel,
};

fn image(width: u32, height: u32) -> CpuImage {
    CpuImage::new(
        width,
        height,
        vec![RgbaPixel::new(-0.25, 0.5, 4.0, 0.65).unwrap(); (width * height) as usize],
    )
    .unwrap()
}

fn mask(id: &str) -> RadialMask {
    RadialMask {
        id: id.into(),
        enabled: true,
        center_x: 0.5,
        center_y: 0.5,
        radius_x: 0.3,
        radius_y: 0.2,
        rotation_degrees: 25.0,
        feather: 0.35,
        opacity: 0.8,
        invert: false,
        adjustments: LocalAdjustments {
            exposure_ev: 0.0,
            brightness: 20.0,
            contrast: -10.0,
            saturation: 15.0,
            temperature: 8.0,
            tint: -4.0,
            sharpness: 12.0,
        },
    }
}

fn full_mask(adjustments: LocalAdjustments) -> RadialMask {
    RadialMask {
        id: "full".into(),
        enabled: true,
        center_x: 0.5,
        center_y: 0.5,
        radius_x: 2.0,
        radius_y: 2.0,
        rotation_degrees: 0.0,
        feather: 0.0,
        opacity: 1.0,
        invert: false,
        adjustments,
    }
}

#[test]
fn radial_local_adjustment_is_deterministic_preserves_alpha_and_hdr() {
    let mut first = image(31, 23);
    let mut second = first.clone();
    let mut settings = DevelopSettings::default();
    settings.radial_masks.masks.push(mask("primary"));
    DevelopPipeline.process(&mut first, &settings).unwrap();
    DevelopPipeline.process(&mut second, &settings).unwrap();
    assert_eq!(first, second);
    assert!(first.pixels().iter().all(|pixel| pixel.alpha() == 0.65));
    assert!(first.pixels().iter().all(|pixel| {
        pixel.red().is_finite() && pixel.green().is_finite() && pixel.blue().is_finite()
    }));
    assert!(first.pixels().iter().any(|pixel| pixel.blue() > 4.0));
}

#[test]
fn disabled_zero_opacity_and_neutral_masks_are_bit_exact() {
    let source = image(9, 7);
    for candidate in {
        let mut disabled = mask("disabled");
        disabled.enabled = false;
        let mut transparent = mask("transparent");
        transparent.opacity = 0.0;
        let mut neutral = mask("neutral");
        neutral.adjustments = LocalAdjustments::default();
        [disabled, transparent, neutral]
    } {
        let mut rendered = source.clone();
        let mut settings = DevelopSettings::default();
        settings.radial_masks.masks.push(candidate);
        DevelopPipeline.process(&mut rendered, &settings).unwrap();
        assert_eq!(rendered, source);
    }
}

#[test]
fn invert_changes_outside_instead_of_inside() {
    let source = image(21, 21);
    let mut normal = source.clone();
    let mut inverted = source;
    let mut normal_settings = DevelopSettings::default();
    let mut inverted_settings = DevelopSettings::default();
    let mut normal_mask = mask("normal");
    normal_mask.rotation_degrees = 0.0;
    normal_mask.feather = 0.0;
    normal_mask.opacity = 1.0;
    let mut inverted_mask = normal_mask.clone();
    inverted_mask.id = "inverted".into();
    inverted_mask.invert = true;
    normal_settings.radial_masks.masks.push(normal_mask);
    inverted_settings.radial_masks.masks.push(inverted_mask);
    DevelopPipeline
        .process(&mut normal, &normal_settings)
        .unwrap();
    DevelopPipeline
        .process(&mut inverted, &inverted_settings)
        .unwrap();
    let center = 10 * 21 + 10;
    let corner = 0;
    assert!(normal.pixels()[center].blue() > inverted.pixels()[center].blue());
    assert!(normal.pixels()[corner].blue() < inverted.pixels()[corner].blue());
}

#[test]
fn every_local_point_slider_is_bit_exact_to_wp1_global_math() {
    for field in [
        "exposure_ev",
        "brightness",
        "contrast",
        "saturation",
        "temperature",
        "tint",
    ] {
        let source = CpuImage::new(
            7,
            5,
            (0..35)
                .map(|index| {
                    let value = index as f32 / 11.0;
                    RgbaPixel::new(-0.4 + value, 0.2 + value * 0.3, 3.0 - value * 0.2, 0.73)
                        .unwrap()
                })
                .collect(),
        )
        .unwrap();
        let mut global = source.clone();
        let mut local = source;
        let mut global_settings = DevelopSettings::default();
        let mut adjustments = LocalAdjustments::default();
        match field {
            "exposure_ev" => {
                global_settings.basics.exposure_ev = 1.75;
                adjustments.exposure_ev = 1.75;
            }
            "brightness" => {
                global_settings.basics.brightness = 37.0;
                adjustments.brightness = 37.0;
            }
            "contrast" => {
                global_settings.basics.contrast = 37.0;
                adjustments.contrast = 37.0;
            }
            "saturation" => {
                global_settings.basics.saturation = 37.0;
                adjustments.saturation = 37.0;
            }
            "temperature" => {
                global_settings.basics.temperature = 37.0;
                adjustments.temperature = 37.0;
            }
            "tint" => {
                global_settings.basics.tint = 37.0;
                adjustments.tint = 37.0;
            }
            _ => unreachable!(),
        }
        let mut local_settings = DevelopSettings::default();
        local_settings
            .radial_masks
            .masks
            .push(full_mask(adjustments));
        DevelopPipeline
            .process(&mut global, &global_settings)
            .unwrap();
        DevelopPipeline
            .process(&mut local, &local_settings)
            .unwrap();
        assert_eq!(local, global, "local {field} diverged from WP1");
    }
}

#[test]
fn local_exposure_precedes_brightness_and_contrast_exactly_like_global_basics() {
    let source = CpuImage::new(
        5,
        1,
        [-0.5, 0.0, 0.18, 1.0, 3.0]
            .into_iter()
            .map(|value| RgbaPixel::new(value, value * 0.7, value * 1.3, 0.42).unwrap())
            .collect(),
    )
    .unwrap();
    let mut global = source.clone();
    let mut local = source;
    let mut global_settings = DevelopSettings::default();
    global_settings.basics.exposure_ev = -1.25;
    global_settings.basics.brightness = 37.0;
    global_settings.basics.contrast = 62.0;
    let adjustments = LocalAdjustments {
        exposure_ev: -1.25,
        brightness: 37.0,
        contrast: 62.0,
        ..LocalAdjustments::default()
    };
    let mut local_settings = DevelopSettings::default();
    local_settings
        .radial_masks
        .masks
        .push(full_mask(adjustments));
    DevelopPipeline
        .process(&mut global, &global_settings)
        .unwrap();
    DevelopPipeline
        .process(&mut local, &local_settings)
        .unwrap();
    assert_eq!(local, global);
}

#[test]
fn local_exposure_feather_invert_and_two_mask_order_are_deterministic() {
    let source = CpuImage::new(
        19,
        13,
        (0..247)
            .map(|index| {
                let value = -0.3 + index as f32 / 53.0;
                RgbaPixel::new(value, value * 0.8, value * 1.4, 0.31).unwrap()
            })
            .collect(),
    )
    .unwrap();
    let mut first = mask("first");
    first.adjustments = LocalAdjustments {
        exposure_ev: 1.25,
        contrast: 18.0,
        ..LocalAdjustments::default()
    };
    first.feather = 0.7;
    let mut second = mask("second");
    second.adjustments = LocalAdjustments {
        exposure_ev: -0.75,
        brightness: 21.0,
        ..LocalAdjustments::default()
    };
    second.center_x = 0.68;
    second.invert = true;
    let render = |masks: Vec<RadialMask>| {
        let mut rendered = source.clone();
        let mut settings = DevelopSettings::default();
        settings.radial_masks.masks = masks;
        DevelopPipeline.process(&mut rendered, &settings).unwrap();
        rendered
    };
    let forward = render(vec![first.clone(), second.clone()]);
    assert_eq!(forward, render(vec![first.clone(), second.clone()]));
    let reverse = render(vec![second, first]);
    assert_ne!(
        forward, reverse,
        "mask layers must preserve persisted order"
    );
    assert!(forward.pixels().iter().all(|pixel| pixel.alpha() == 0.31));
    assert!(forward.pixels().iter().all(|pixel| {
        pixel.red().is_finite() && pixel.green().is_finite() && pixel.blue().is_finite()
    }));
}

#[test]
fn two_local_exposure_masks_shape_a_synthetic_lit_scene() {
    let mut rendered = CpuImage::new(
        25,
        25,
        vec![RgbaPixel::new(0.12, 0.1, 0.08, 1.0).unwrap(); 625],
    )
    .unwrap();
    let mut settings = DevelopSettings::default();
    for (id, center_x, exposure_ev) in [("left", 0.32, 2.0), ("right", 0.72, -1.0)] {
        let mut radial = full_mask(LocalAdjustments {
            exposure_ev,
            ..LocalAdjustments::default()
        });
        radial.id = id.to_owned();
        radial.center_x = center_x;
        radial.radius_x = 0.22;
        radial.radius_y = 0.4;
        radial.feather = 0.65;
        settings.radial_masks.masks.push(radial);
    }
    DevelopPipeline.process(&mut rendered, &settings).unwrap();
    let left = rendered.pixels()[12 * 25 + 8].red();
    let right = rendered.pixels()[12 * 25 + 18].red();
    assert!(left > 0.12);
    assert!(right < left);
}

#[test]
fn local_sharpness_is_bit_exact_to_wp3_global_usm() {
    let source = CpuImage::new(
        9,
        7,
        (0..63)
            .map(|index| {
                let edge = if index % 9 < 4 { -0.5 } else { 3.0 };
                RgbaPixel::new(edge, edge * 0.5, edge * 1.5, 0.61).unwrap()
            })
            .collect(),
    )
    .unwrap();
    let mut global = source.clone();
    let mut local = source;
    let mut global_settings = DevelopSettings::default();
    global_settings.effects.sharpness = 64.0;
    let adjustments = LocalAdjustments {
        sharpness: 64.0,
        ..LocalAdjustments::default()
    };
    let mut local_settings = DevelopSettings::default();
    local_settings
        .radial_masks
        .masks
        .push(full_mask(adjustments));
    DevelopPipeline
        .process(&mut global, &global_settings)
        .unwrap();
    DevelopPipeline
        .process(&mut local, &local_settings)
        .unwrap();
    assert_eq!(local, global);
}

#[test]
fn local_pipeline_builds_the_full_effect_before_mask_mix() {
    let source = image(11, 9);
    let mut globally_adjusted = source.clone();
    let mut global_settings = DevelopSettings::default();
    global_settings.basics.brightness = 25.0;
    global_settings.effects.sharpness = 50.0;
    DevelopPipeline
        .process(&mut globally_adjusted, &global_settings)
        .unwrap();

    let adjustments = LocalAdjustments {
        brightness: 25.0,
        sharpness: 50.0,
        ..LocalAdjustments::default()
    };
    let mut radial = full_mask(adjustments);
    radial.opacity = 0.5;
    let mut settings = DevelopSettings::default();
    settings.radial_masks.masks.push(radial);
    let mut local = source.clone();
    DevelopPipeline.process(&mut local, &settings).unwrap();
    let center = 4 * 11 + 5;
    for (actual, original, adjusted) in [
        (
            local.pixels()[center].red(),
            source.pixels()[center].red(),
            globally_adjusted.pixels()[center].red(),
        ),
        (
            local.pixels()[center].green(),
            source.pixels()[center].green(),
            globally_adjusted.pixels()[center].green(),
        ),
        (
            local.pixels()[center].blue(),
            source.pixels()[center].blue(),
            globally_adjusted.pixels()[center].blue(),
        ),
    ] {
        assert!((actual - (original + 0.5 * (adjusted - original))).abs() < 1.0e-6);
    }
}

#[test]
fn negative_local_sharpness_is_loudly_unsupported_and_atomic() {
    let mut rendered = image(5, 5);
    let original = rendered.clone();
    let adjustments = LocalAdjustments {
        sharpness: -1.0,
        ..LocalAdjustments::default()
    };
    let mut settings = DevelopSettings::default();
    settings.radial_masks.masks.push(full_mask(adjustments));
    assert_eq!(
        DevelopPipeline.process(&mut rendered, &settings),
        Err(PipelineError::StageNotImplemented(
            DevelopStage::RadialMasks
        ))
    );
    assert_eq!(rendered, original);
}

#[test]
fn local_exposure_range_and_overflow_fail_atomically() {
    for exposure_ev in [-4.0, 4.0] {
        let mut settings = DevelopSettings::default();
        settings
            .radial_masks
            .masks
            .push(full_mask(LocalAdjustments {
                exposure_ev,
                ..LocalAdjustments::default()
            }));
        settings.validate().unwrap();
    }
    for exposure_ev in [-4.01, 4.01, f32::NAN, f32::INFINITY] {
        let mut settings = DevelopSettings::default();
        settings
            .radial_masks
            .masks
            .push(full_mask(LocalAdjustments {
                exposure_ev,
                ..LocalAdjustments::default()
            }));
        assert!(settings.validate().is_err());
    }

    let mut rendered =
        CpuImage::new(1, 1, vec![RgbaPixel::new(f32::MAX, 1.0, 1.0, 0.8).unwrap()]).unwrap();
    let original = rendered.clone();
    let mut settings = DevelopSettings::default();
    settings
        .radial_masks
        .masks
        .push(full_mask(LocalAdjustments {
            exposure_ev: 4.0,
            ..LocalAdjustments::default()
        }));
    assert!(DevelopPipeline.process(&mut rendered, &settings).is_err());
    assert_eq!(rendered, original);
}
