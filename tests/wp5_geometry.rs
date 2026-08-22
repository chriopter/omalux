use grainroom::develop::{CpuImage, CropRect, DevelopPipeline, DevelopSettings, RgbaPixel};

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
                    0.25 + 0.05 * value as f32,
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
