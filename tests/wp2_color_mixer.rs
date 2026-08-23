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

#[test]
fn non_neutral_color_mixer_is_supported_and_preserves_alpha() {
    let mut settings = DevelopSettings::default();
    settings.color_mixer.red.hue_shift_degrees = 30.0;
    settings.color_mixer.red.saturation = 25.0;

    let mut rendered = image([1.0, 0.0, 0.0, 0.37]);
    DevelopPipeline.process(&mut rendered, &settings).unwrap();
    let pixel = rendered.pixels()[0];
    assert_eq!(pixel.alpha(), 0.37);
    assert_ne!([pixel.red(), pixel.green(), pixel.blue()], [1.0, 0.0, 0.0]);
}

#[test]
fn neutral_mixer_is_bit_exact_for_negative_and_hdr_pixels() {
    let settings = DevelopSettings::default();
    let mut rendered = image([-0.25, 2.0, 16.0, 1.0]);
    let original = rendered.clone();
    DevelopPipeline.process(&mut rendered, &settings).unwrap();
    assert_eq!(rendered, original);
}

#[test]
fn arbitrary_valid_mixer_settings_remain_finite() {
    let mut state = 0x1234_5678_u32;
    for _ in 0..512 {
        state = state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        let sample = |bits: u32| bits as f32 / u32::MAX as f32;
        let rgb = [
            -0.25 + 16.25 * sample(state),
            -0.25 + 16.25 * sample(state.rotate_left(9)),
            -0.25 + 16.25 * sample(state.rotate_left(19)),
            1.0,
        ];
        let mut settings = DevelopSettings::default();
        settings.color_mixer.blue.hue_shift_degrees = sample(state.rotate_left(5)) * 360.0 - 180.0;
        settings.color_mixer.blue.saturation = sample(state.rotate_left(13)) * 200.0 - 100.0;
        settings.color_mixer.blue.luminance = sample(state.rotate_left(23)) * 200.0 - 100.0;
        let mut rendered = image(rgb);
        DevelopPipeline.process(&mut rendered, &settings).unwrap();
        let pixel = rendered.pixels()[0];
        assert!(pixel.red().is_finite());
        assert!(pixel.green().is_finite());
        assert!(pixel.blue().is_finite());
    }
}

#[test]
fn signed_counterexample_preserves_actual_target_luminance() {
    let mut settings = DevelopSettings::default();
    for band in [
        &mut settings.color_mixer.red,
        &mut settings.color_mixer.orange,
        &mut settings.color_mixer.yellow,
        &mut settings.color_mixer.green,
        &mut settings.color_mixer.aqua,
        &mut settings.color_mixer.blue,
        &mut settings.color_mixer.purple,
        &mut settings.color_mixer.magenta,
    ] {
        band.hue_shift_degrees = 176.986_14;
        band.saturation = 75.408_96;
    }
    let rgb = [0.186_149_76, -0.214_877_37, 0.247_390_31];
    let target = rgb[0] * 0.262_700_2 + rgb[1] * 0.677_998_1 + rgb[2] * 0.059_301_7;
    let mut rendered = image([rgb[0], rgb[1], rgb[2], 0.61]);
    DevelopPipeline.process(&mut rendered, &settings).unwrap();
    let pixel = rendered.pixels()[0];
    let actual =
        pixel.red() * 0.262_700_2 + pixel.green() * 0.677_998_1 + pixel.blue() * 0.059_301_7;
    assert!((actual - target).abs() <= 2.0e-6);
    assert_eq!(pixel.alpha(), 0.61);
}

#[test]
fn broad_signed_hdr_pipeline_reaches_the_requested_y() {
    let mut state = 0xa341_316c_u32;
    for _ in 0..512 {
        state = state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        let sample = |value: u32| value as f32 / u32::MAX as f32;
        let rgb = [
            -2.0 + 18.0 * sample(state),
            -2.0 + 18.0 * sample(state.rotate_left(9)),
            -2.0 + 18.0 * sample(state.rotate_left(21)),
        ];
        let hue = -180.0 + 360.0 * sample(state.rotate_left(3));
        let saturation = -100.0 + 200.0 * sample(state.rotate_left(13));
        let luminance = -100.0 + 200.0 * sample(state.rotate_left(25));
        let mut settings = DevelopSettings::default();
        for band in [
            &mut settings.color_mixer.red,
            &mut settings.color_mixer.orange,
            &mut settings.color_mixer.yellow,
            &mut settings.color_mixer.green,
            &mut settings.color_mixer.aqua,
            &mut settings.color_mixer.blue,
            &mut settings.color_mixer.purple,
            &mut settings.color_mixer.magenta,
        ] {
            band.hue_shift_degrees = hue;
            band.saturation = saturation;
            band.luminance = luminance;
        }
        let source_y = rgb[0] * 0.262_700_2 + rgb[1] * 0.677_998_1 + rgb[2] * 0.059_301_7;
        let target = source_y * (2.0 * luminance / 100.0).exp2();
        let mut rendered = image([rgb[0], rgb[1], rgb[2], 0.73]);
        DevelopPipeline.process(&mut rendered, &settings).unwrap();
        let pixel = rendered.pixels()[0];
        let actual =
            pixel.red() * 0.262_700_2 + pixel.green() * 0.677_998_1 + pixel.blue() * 0.059_301_7;
        assert!((actual - target).abs() <= 2.0e-5 * (1.0 + target.abs()));
        assert_eq!(pixel.alpha(), 0.73);
    }
}

#[test]
fn unrepresentable_positive_ev_target_is_transactional() {
    let mut settings = DevelopSettings::default();
    for band in [
        &mut settings.color_mixer.red,
        &mut settings.color_mixer.orange,
        &mut settings.color_mixer.yellow,
        &mut settings.color_mixer.green,
        &mut settings.color_mixer.aqua,
        &mut settings.color_mixer.blue,
        &mut settings.color_mixer.purple,
        &mut settings.color_mixer.magenta,
    ] {
        band.luminance = 100.0;
    }
    let mut rendered = image([f32::MAX, f32::MAX * 0.5, f32::MAX * 0.25, 0.44]);
    let original = rendered.clone();
    assert!(matches!(
        DevelopPipeline.process(&mut rendered, &settings),
        Err(PipelineError::NumericFailure {
            stage: DevelopStage::ColorMixer,
            ..
        })
    ));
    assert_eq!(rendered, original);
}

#[test]
fn exponent_and_cancellation_sweep_never_silently_misses_y() {
    let mut settings = DevelopSettings::default();
    for band in [
        &mut settings.color_mixer.red,
        &mut settings.color_mixer.orange,
        &mut settings.color_mixer.yellow,
        &mut settings.color_mixer.green,
        &mut settings.color_mixer.aqua,
        &mut settings.color_mixer.blue,
        &mut settings.color_mixer.purple,
        &mut settings.color_mixer.magenta,
    ] {
        band.hue_shift_degrees = 179.0;
        band.saturation = 100.0;
    }
    for magnitude in [
        f32::from_bits(1),
        1.0e-30,
        1.0e-10,
        1.0,
        1.0e10,
        1.0e30,
        f32::MAX,
    ] {
        let red = magnitude;
        let green = -(f64::from(red) * 0.262_700_2_f64 / 0.677_998_1_f64) as f32;
        let rgb = [red, green, magnitude * 0.03125];
        let mut rendered = image([rgb[0], rgb[1], rgb[2], 0.52]);
        let original = rendered.clone();
        let target = f64::from(rgb[0]) * 0.262_700_2
            + f64::from(rgb[1]) * 0.677_998_1
            + f64::from(rgb[2]) * 0.059_301_7;
        match DevelopPipeline.process(&mut rendered, &settings) {
            Ok(()) => {
                let pixel = rendered.pixels()[0];
                let actual = f64::from(pixel.red()) * 0.262_700_2
                    + f64::from(pixel.green()) * 0.677_998_1
                    + f64::from(pixel.blue()) * 0.059_301_7;
                let weighted_magnitude = f64::from(pixel.red()).abs() * 0.262_700_2
                    + f64::from(pixel.green()).abs() * 0.677_998_1
                    + f64::from(pixel.blue()).abs() * 0.059_301_7;
                let tolerance = (64.0 * f64::from(f32::EPSILON) * target.abs())
                    .max(8.0 * f64::from(f32::EPSILON) * weighted_magnitude);
                assert!((actual - target).abs() <= tolerance);
                assert_eq!(pixel.alpha(), 0.52);
            }
            Err(PipelineError::NumericFailure {
                stage: DevelopStage::ColorMixer,
                ..
            }) => assert_eq!(rendered, original),
            Err(error) => panic!("unexpected pipeline error: {error}"),
        }
    }
}
