use omalux::develop::{
    CpuImage, DevelopPipeline, DevelopSettings, DevelopStage, PipelineError, RgbaPixel,
};

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

#[test]
fn signed_counterexample_preserves_actual_luminance() {
    let mut settings = DevelopSettings::default();
    settings.color_grading.shadows.hue_degrees = 218.021_76;
    settings.color_grading.shadows.saturation = 77.775_41;
    let rgb = [0.004_847_942, -0.226_648_48, 1.279_416_7];
    let mut rendered = image([rgb[0], rgb[1], rgb[2], 0.29]);
    let target = luminance(&rendered.pixels()[0]);
    DevelopPipeline.process(&mut rendered, &settings).unwrap();
    let pixel = rendered.pixels()[0];
    assert!((luminance(&pixel) - target).abs() <= 2.0e-6);
    assert_eq!(pixel.alpha(), 0.29);
}

#[test]
fn black_can_receive_chroma_without_a_false_zero_luminance_failure() {
    let mut settings = DevelopSettings::default();
    settings.color_grading.shadows.hue_degrees = 32.0;
    settings.color_grading.shadows.saturation = 45.0;
    let mut rendered = image([0.0, 0.0, 0.0, 1.0]);
    DevelopPipeline.process(&mut rendered, &settings).unwrap();
    let pixel = rendered.pixels()[0];
    assert!(pixel.red().is_finite());
    assert!(pixel.green().is_finite());
    assert!(pixel.blue().is_finite());
    assert!(luminance(&pixel).abs() <= 1.0e-6);
}

#[test]
fn broad_signed_hdr_pipeline_reaches_the_requested_y() {
    let mut state = 0xc801_3ea4_u32;
    for _ in 0..512 {
        state = state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        let sample = |value: u32| value as f32 / u32::MAX as f32;
        let rgb = [
            -2.0 + 18.0 * sample(state),
            -2.0 + 18.0 * sample(state.rotate_left(9)),
            -2.0 + 18.0 * sample(state.rotate_left(21)),
        ];
        let hue = 360.0 * sample(state.rotate_left(3));
        let saturation = 100.0 * sample(state.rotate_left(13));
        let luminance_amount = -100.0 + 200.0 * sample(state.rotate_left(25));
        let mut settings = DevelopSettings::default();
        for range in [
            &mut settings.color_grading.shadows,
            &mut settings.color_grading.midtones,
            &mut settings.color_grading.highlights,
        ] {
            range.hue_degrees = hue;
            range.saturation = saturation;
            range.luminance = luminance_amount;
        }
        settings.color_grading.balance = -100.0 + 200.0 * sample(state.rotate_left(5));
        settings.color_grading.blending = 100.0 * sample(state.rotate_left(17));
        let source_y = rgb[0] * 0.262_700_2 + rgb[1] * 0.677_998_1 + rgb[2] * 0.059_301_7;
        let target = source_y * (2.0 * luminance_amount / 100.0).exp2();
        let mut rendered = image([rgb[0], rgb[1], rgb[2], 0.81]);
        DevelopPipeline.process(&mut rendered, &settings).unwrap();
        let pixel = rendered.pixels()[0];
        assert!((luminance(&pixel) - target).abs() <= 2.0e-5 * (1.0 + target.abs()));
        assert_eq!(pixel.alpha(), 0.81);
    }
}

#[test]
fn unrepresentable_positive_ev_target_is_transactional() {
    let mut settings = DevelopSettings::default();
    settings.color_grading.shadows.luminance = 100.0;
    settings.color_grading.midtones.luminance = 100.0;
    settings.color_grading.highlights.luminance = 100.0;
    let mut rendered = image([f32::MAX, f32::MAX * 0.5, f32::MAX * 0.25, 0.36]);
    let original = rendered.clone();
    assert!(matches!(
        DevelopPipeline.process(&mut rendered, &settings),
        Err(PipelineError::NumericFailure {
            stage: DevelopStage::ColorGrading,
            ..
        })
    ));
    assert_eq!(rendered, original);
}
