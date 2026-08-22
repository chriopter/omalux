use grainroom::develop::{
    CpuImage, DevelopPipeline, DevelopSettings, LocalAdjustments, RadialMask, RgbaPixel,
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
            brightness: 20.0,
            contrast: -10.0,
            saturation: 15.0,
            temperature: 8.0,
            tint: -4.0,
            sharpness: 12.0,
        },
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
