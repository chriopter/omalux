use omalux::develop::{
    CpuImage, CropRect, DevelopPipeline, DevelopSettings, RgbaPixel, parameter_registry,
};

fn numbered(width: u32, height: u32) -> CpuImage {
    CpuImage::new(
        width,
        height,
        (0..width * height)
            .map(|value| {
                RgbaPixel::new(
                    value as f32 - 2.0,
                    value as f32 * 2.0,
                    4.0 - value as f32,
                    0.25 + 0.05 * (value % 10) as f32,
                )
                .unwrap()
            })
            .collect(),
    )
    .unwrap()
}

#[test]
fn identity_and_full_crop_are_bit_exact() {
    let source = numbered(3, 2);
    let mut rendered = source.clone();
    let mut settings = DevelopSettings::default();
    settings.geometry.crop = Some(CropRect {
        x: 0.0,
        y: 0.0,
        width: 1.0,
        height: 1.0,
    });
    DevelopPipeline.process(&mut rendered, &settings).unwrap();
    assert_eq!(rendered, source);
}

#[test]
fn quarter_turn_flip_and_normalized_crop_are_exact() {
    let mut rendered = numbered(4, 3);
    let mut settings = DevelopSettings::default();
    settings.geometry.quarter_turns_clockwise = 1;
    settings.geometry.flip_horizontal = true;
    settings.geometry.crop = Some(CropRect {
        x: 0.0,
        y: 0.25,
        width: 1.0,
        height: 0.5,
    });
    DevelopPipeline.process(&mut rendered, &settings).unwrap();
    assert_eq!((rendered.width(), rendered.height()), (3, 2));
    assert_eq!(
        rendered
            .pixels()
            .iter()
            .map(RgbaPixel::red)
            .collect::<Vec<_>>(),
        vec![-1.0, 3.0, 7.0, 0.0, 4.0, 8.0]
    );
}

#[test]
fn projective_render_is_deterministic_finite_and_preserves_hdr_alpha_contract() {
    let source = CpuImage::new(
        7,
        5,
        (0..35)
            .map(|index| {
                let value = index as f32 / 7.0;
                RgbaPixel::new(
                    -1.0 + value,
                    2.0 * value,
                    8.0 - value,
                    (index % 5) as f32 / 4.0,
                )
                .unwrap()
            })
            .collect(),
    )
    .unwrap();
    let mut first = source.clone();
    let mut second = source;
    let mut settings = DevelopSettings::default();
    settings.geometry.straighten_degrees = 7.25;
    settings.geometry.perspective_horizontal = 18.0;
    settings.geometry.perspective_vertical = -11.0;
    DevelopPipeline.process(&mut first, &settings).unwrap();
    DevelopPipeline.process(&mut second, &settings).unwrap();
    assert_eq!(first, second);
    assert!(first.pixels().iter().all(|pixel| {
        pixel.red().is_finite()
            && pixel.green().is_finite()
            && pixel.blue().is_finite()
            && (0.0..=1.0).contains(&pixel.alpha())
    }));
    assert!(first.pixels().iter().any(|pixel| pixel.blue() > 1.0));
}

#[test]
fn crop_boundaries_are_strict_and_report_the_precise_field() {
    for (crop, expected_path) in [
        (
            CropRect {
                x: 1.0,
                y: 0.0,
                width: f32::from_bits(1),
                height: 1.0,
            },
            "geometry.crop.x",
        ),
        (
            CropRect {
                x: 0.0,
                y: 1.0,
                width: 1.0,
                height: f32::from_bits(1),
            },
            "geometry.crop.y",
        ),
        (
            CropRect {
                x: 0.0,
                y: 0.0,
                width: 0.0,
                height: 1.0,
            },
            "geometry.crop.width",
        ),
        (
            CropRect {
                x: 0.75,
                y: 0.0,
                width: 0.250_000_03,
                height: 1.0,
            },
            "geometry.crop.width",
        ),
    ] {
        let mut settings = DevelopSettings::default();
        settings.geometry.crop = Some(crop);
        assert_eq!(settings.validate().unwrap_err().path(), expected_path);
    }
}

#[test]
fn crop_registry_matches_half_open_origin_and_positive_extent_contract() {
    let registry = parameter_registry();
    let definition = |id: &str| registry.iter().find(|entry| entry.id == id).unwrap();
    assert_eq!(
        definition("geometry.crop.x").maximum.to_bits(),
        1.0_f32.to_bits() - 1
    );
    assert_eq!(
        definition("geometry.crop.y").maximum.to_bits(),
        1.0_f32.to_bits() - 1
    );
    assert_eq!(definition("geometry.crop.width").minimum.to_bits(), 1);
    assert_eq!(definition("geometry.crop.height").minimum.to_bits(), 1);
}

#[test]
fn arbitrarily_small_valid_crop_yields_one_in_bounds_pixel() {
    let mut rendered = numbered(8, 6);
    let mut settings = DevelopSettings::default();
    settings.geometry.crop = Some(CropRect {
        x: 0.875,
        y: 5.0 / 6.0,
        width: f32::from_bits(1),
        height: f32::from_bits(1),
    });
    DevelopPipeline.process(&mut rendered, &settings).unwrap();
    assert_eq!((rendered.width(), rendered.height()), (1, 1));
    assert_eq!(rendered.pixels()[0].red(), 37.0);
}

#[test]
fn fractional_crop_uses_floor_lower_and_ceil_upper_edges() {
    let mut rendered = numbered(10, 10);
    let mut settings = DevelopSettings::default();
    settings.geometry.crop = Some(CropRect {
        x: 0.11,
        y: 0.21,
        width: 0.22,
        height: 0.31,
    });
    DevelopPipeline.process(&mut rendered, &settings).unwrap();
    assert_eq!((rendered.width(), rendered.height()), (3, 4));
    assert_eq!(rendered.pixels()[0].red(), 19.0);
}
