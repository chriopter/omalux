use grainroom::develop::{
    CpuImage, DevelopPipeline, DevelopSettings, DevelopStage, PipelineError, RgbaPixel,
};

const LUMA: [f64; 3] = [0.262_700_2, 0.677_998_1, 0.059_301_7];

#[test]
fn neutral_basics_is_bit_exact() {
    let mut image = image([-0.25, 0.5, 4.0]);
    let original = image.clone();
    DevelopPipeline
        .process(&mut image, &DevelopSettings::default())
        .unwrap();
    assert_eq!(image, original);
}

#[test]
fn brightness_endpoints_are_plus_and_minus_one_ev() {
    assert_rgb_close(
        render_gray(0.18, |settings| settings.basics.brightness = 100.0),
        0.36,
    );
    assert_rgb_close(
        render_gray(0.18, |settings| settings.basics.brightness = -100.0),
        0.09,
    );
}

#[test]
fn contrast_uses_the_eighteen_percent_fulcrum() {
    for (amount, input, expected) in [
        (100.0, 0.045, 0.01125),
        (100.0, 0.72, 2.88),
        (-100.0, 0.045, 0.09),
        (-100.0, 0.72, 0.36),
        (100.0, 0.18, 0.18),
    ] {
        assert_rgb_close(
            render_gray(input, |settings| settings.basics.contrast = amount),
            expected,
        );
    }
}

#[test]
fn saturation_and_vibrance_preserve_rec2020_luminance() {
    let input = [0.8, 0.2, 0.05];
    let input_luma = luma(input);
    let saturated = render(input, |settings| settings.basics.saturation = 70.0);
    let vibrant = render(input, |settings| settings.basics.vibrance = 70.0);
    assert_close(luma(saturated), input_luma, 2.0e-6);
    assert_close(luma(vibrant), input_luma, 2.0e-6);
    assert!((saturated[0] - saturated[2]).abs() > (input[0] - input[2]).abs());
    assert!((vibrant[0] - vibrant[2]).abs() >= (input[0] - input[2]).abs());
}

#[test]
fn tonal_masks_are_global_luminance_gains() {
    let shadow = render_gray(0.02, |settings| settings.basics.shadows = 100.0);
    let highlight = render_gray(2.0, |settings| settings.basics.highlights = -100.0);
    let black = render_gray(0.005, |settings| settings.basics.blacks = 100.0);
    let white = render_gray(4.0, |settings| settings.basics.whites = -100.0);
    assert!(shadow[0] > 0.02);
    assert!(highlight[0] < 2.0);
    assert!(black[0] > 0.005);
    assert!(white[0] < 4.0);
}

#[test]
fn temperature_and_tint_are_prepared_finite_matrices() {
    let warm = render([0.18, 0.18, 0.18], |settings| {
        settings.basics.temperature = 50.0
    });
    let tinted = render([0.18, 0.18, 0.18], |settings| settings.basics.tint = 50.0);
    assert!(warm.into_iter().all(f64::is_finite));
    assert!(tinted.into_iter().all(f64::is_finite));
    assert_ne!(warm, [0.18_f64; 3]);
    assert_ne!(tinted, [0.18_f64; 3]);
}

#[test]
fn temperature_and_tint_follow_conventional_directions() {
    let warm = render_gray(0.18, |settings| settings.basics.temperature = 100.0);
    let cool = render_gray(0.18, |settings| settings.basics.temperature = -100.0);
    let magenta = render_gray(0.18, |settings| settings.basics.tint = 100.0);
    assert!(warm[0] > warm[2], "positive temperature must be warmer");
    assert!(cool[2] > cool[0], "negative temperature must be cooler");
    assert!(
        magenta[1] < magenta[0] && magenta[1] < magenta[2],
        "positive tint must move toward magenta"
    );

    let almost_warm = render_gray(0.18, |settings| settings.basics.temperature = 96.0);
    assert_ne!(
        almost_warm, warm,
        "the warm endpoint must have no dead zone"
    );
}

#[test]
fn negative_and_hdr_values_remain_finite_and_unclipped() {
    let output = render([-0.5, 0.25, 8.0], |settings| {
        settings.basics.brightness = 50.0;
        settings.basics.saturation = 20.0;
    });
    assert!(output.into_iter().all(f64::is_finite));
    assert!(output[0] < 0.0);
    assert!(output[2] > 1.0);
}

#[test]
fn clarity_remains_an_explicit_unsupported_capability() {
    let mut settings = DevelopSettings::default();
    settings.basics.clarity = 1.0;
    assert_eq!(
        DevelopPipeline.preflight(&settings),
        Err(PipelineError::StageNotImplemented(DevelopStage::Basics))
    );
}

fn render_gray(value: f64, configure: impl FnOnce(&mut DevelopSettings)) -> [f64; 3] {
    render([value; 3], configure)
}

fn render(rgb: [f64; 3], configure: impl FnOnce(&mut DevelopSettings)) -> [f64; 3] {
    let mut settings = DevelopSettings::default();
    configure(&mut settings);
    let mut image = image(rgb);
    DevelopPipeline.process(&mut image, &settings).unwrap();
    let pixel = image.pixels()[0];
    [
        f64::from(pixel.red()),
        f64::from(pixel.green()),
        f64::from(pixel.blue()),
    ]
}

fn image(rgb: [f64; 3]) -> CpuImage {
    CpuImage::new(
        1,
        1,
        vec![RgbaPixel::new(rgb[0] as f32, rgb[1] as f32, rgb[2] as f32, 0.75).unwrap()],
    )
    .unwrap()
}

fn luma(rgb: [f64; 3]) -> f64 {
    rgb[0] * LUMA[0] + rgb[1] * LUMA[1] + rgb[2] * LUMA[2]
}

fn assert_rgb_close(actual: [f64; 3], expected: f64) {
    for channel in actual {
        assert_close(channel, expected, 2.0e-6);
    }
}

fn assert_close(actual: f64, expected: f64, tolerance: f64) {
    assert!(
        (actual - expected).abs() <= tolerance,
        "expected {expected}, got {actual}"
    );
}
