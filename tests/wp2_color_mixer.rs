use grainroom::develop::{CpuImage, DevelopPipeline, DevelopSettings, RgbaPixel};

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
