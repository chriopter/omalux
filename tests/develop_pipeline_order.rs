use grainroom::develop::settings::CurvePoint;
use grainroom::develop::{CpuImage, DevelopPipeline, DevelopSettings, RgbaPixel};

#[test]
fn wp1_wp2_wp3_canonical_order_matches_the_combined_golden() {
    let input = RgbaPixel::new(0.12, 0.28, 0.65, 0.37).unwrap();
    let mut image = CpuImage::new(1, 1, vec![input]).unwrap();
    let mut settings = DevelopSettings::default();

    // WP1: Basics, followed by the master tone curve.
    settings.basics.brightness = 20.0;
    settings.basics.contrast = 15.0;
    settings.basics.saturation = 10.0;
    settings.tone_curves.master.points = vec![
        CurvePoint { x: 0.0, y: 0.0 },
        CurvePoint { x: 0.5, y: 0.58 },
        CurvePoint { x: 1.0, y: 1.0 },
    ];

    // WP2: The mixer precedes three-way grading.
    settings.color_mixer.blue.hue_shift_degrees = 8.0;
    settings.color_mixer.blue.saturation = 12.0;
    settings.color_grading.midtones.hue_degrees = 32.0;
    settings.color_grading.midtones.saturation = 10.0;
    settings.color_grading.midtones.luminance = 5.0;

    // WP3: A non-spatial effect makes the last stage observable without an
    // external fixture or platform-dependent image codec.
    settings.effects.fade = 7.0;

    DevelopPipeline.process(&mut image, &settings).unwrap();
    let output = image.pixels()[0];

    assert_ne!(output, input);
    assert_eq!(output.alpha(), input.alpha());
    assert_close(output.red(), 0.201_657_04);
    assert_close(output.green(), 0.425_947_82);
    assert_close(output.blue(), 1.176_611_8);
}

fn assert_close(actual: f32, expected: f32) {
    assert!(
        (actual - expected).abs() <= 2.0e-6,
        "expected {expected}, got {actual}"
    );
}
