use grainroom::develop::{CpuImage, DevelopPipeline, DevelopSettings, RgbaPixel};

fn image(pixel: [f32; 4]) -> CpuImage {
    CpuImage::new(
        1,
        1,
        vec![RgbaPixel::new(pixel[0], pixel[1], pixel[2], pixel[3]).unwrap()],
    )
    .unwrap()
}

fn luminance(pixel: &RgbaPixel) -> f32 {
    pixel.red() * 0.262_700_2 + pixel.green() * 0.677_998_1 + pixel.blue() * 0.059_301_7
}

#[test]
fn non_neutral_three_way_grade_is_supported_and_preserves_alpha_and_luma() {
    let mut settings = DevelopSettings::default();
    settings.color_grading.midtones.hue_degrees = 210.0;
    settings.color_grading.midtones.saturation = 60.0;
    settings.color_grading.blending = 50.0;

    let mut rendered = image([0.18, 0.18, 0.18, 0.42]);
    let original_y = luminance(&rendered.pixels()[0]);
    DevelopPipeline.process(&mut rendered, &settings).unwrap();
    let pixel = rendered.pixels()[0];
    assert_eq!(pixel.alpha(), 0.42);
    assert!((luminance(&pixel) - original_y).abs() <= 3.0e-6);
    assert_ne!([pixel.red(), pixel.green(), pixel.blue()], [0.18; 3]);
}

#[test]
fn all_zero_grade_is_bit_exact_independent_of_balance_and_blending() {
    let mut settings = DevelopSettings::default();
    settings.color_grading.balance = 100.0;
    settings.color_grading.blending = 100.0;
    let mut rendered = image([-0.25, 2.0, 16.0, 1.0]);
    let original = rendered.clone();
    DevelopPipeline.process(&mut rendered, &settings).unwrap();
    assert_eq!(rendered, original);
}

#[test]
fn valid_grading_settings_remain_finite_for_negative_and_hdr_inputs() {
    let mut settings = DevelopSettings::default();
    settings.color_grading.shadows.hue_degrees = 20.0;
    settings.color_grading.shadows.saturation = 100.0;
    settings.color_grading.shadows.luminance = -100.0;
    settings.color_grading.midtones.hue_degrees = 180.0;
    settings.color_grading.midtones.saturation = 75.0;
    settings.color_grading.highlights.hue_degrees = 320.0;
    settings.color_grading.highlights.saturation = 100.0;
    settings.color_grading.highlights.luminance = 100.0;
    settings.color_grading.balance = -37.0;
    settings.color_grading.blending = 83.0;

    for rgb in [[-0.25, 0.0, 16.0], [-0.01, 0.2, 0.4], [4.0, 2.0, 0.5]] {
        let mut rendered = image([rgb[0], rgb[1], rgb[2], 1.0]);
        DevelopPipeline.process(&mut rendered, &settings).unwrap();
        let pixel = rendered.pixels()[0];
        assert!(pixel.red().is_finite());
        assert!(pixel.green().is_finite());
        assert!(pixel.blue().is_finite());
    }
}
