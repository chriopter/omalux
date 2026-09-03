use omalux::develop::settings::CurvePoint;
use omalux::develop::{
    CpuImage, DevelopPipeline, DevelopRenderContext, DevelopSettings, LocalAdjustments, RadialMask,
    RgbaPixel,
};

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
    // Fade is display-referred (matte print floor and ceiling) and the mixer
    // bands are anchored on their named hues, so these goldens moved with that
    // semantics; the pinned stage order did not.
    assert_close(output.red(), 0.199_159_55);
    assert_close(output.green(), 0.429_619_25);
    assert_close(output.blue(), 0.803_337_34);
}

fn assert_close(actual: f32, expected: f32) {
    assert!(
        (actual - expected).abs() <= 2.0e-6,
        "expected {expected}, got {actual}"
    );
}

#[test]
fn wp1_through_wp5_and_grain_match_the_canonical_order_golden() {
    let pixels = (0..6)
        .map(|index| {
            let index = index as f32;
            RgbaPixel::new(
                0.08 + index * 0.05,
                0.2 + index * 0.03,
                0.65 - index * 0.04,
                0.1 * (index + 1.0),
            )
            .unwrap()
        })
        .collect();
    let mut image = CpuImage::new(3, 2, pixels).unwrap();
    let mut settings = DevelopSettings::default();
    settings.geometry.quarter_turns_clockwise = 1;
    settings.basics.brightness = 12.0;
    settings.basics.contrast = 8.0;
    settings.basics.clarity = 35.0;
    settings.basics.saturation = 5.0;
    settings.tone_curves.master.points = vec![
        CurvePoint { x: 0.0, y: 0.0 },
        CurvePoint { x: 0.5, y: 0.56 },
        CurvePoint { x: 1.0, y: 1.0 },
    ];
    settings.color_mixer.blue.hue_shift_degrees = 7.0;
    settings.color_mixer.blue.saturation = 8.0;
    settings.color_grading.midtones.hue_degrees = 25.0;
    settings.color_grading.midtones.saturation = 6.0;
    settings.color_grading.midtones.luminance = 3.0;
    settings.radial_masks.masks.push(RadialMask {
        id: "order".into(),
        enabled: true,
        center_x: 0.5,
        center_y: 0.5,
        radius_x: 2.0,
        radius_y: 2.0,
        rotation_degrees: 0.0,
        feather: 0.0,
        opacity: 1.0,
        invert: false,
        adjustments: LocalAdjustments {
            brightness: 5.0,
            sharpness: 25.0,
            ..LocalAdjustments::default()
        },
    });
    settings.effects.fade = 4.0;
    settings.effects.grain.amount = 37.0;
    settings.effects.grain.size_iso = 4000.0;
    settings.effects.grain.midtone_response = 80.0;
    let context = DevelopRenderContext::from_source_digest([0x5a; 32]);

    DevelopPipeline
        .process_with_context(&mut image, &settings, Some(&context))
        .unwrap();

    assert_eq!((image.width(), image.height()), (2, 3));
    assert_eq!(
        image
            .pixels()
            .iter()
            .map(|pixel| pixel.alpha().to_bits())
            .collect::<Vec<_>>(),
        vec![
            0.4_f32.to_bits(),
            0.1_f32.to_bits(),
            0.5_f32.to_bits(),
            0.2_f32.to_bits(),
            0.6_f32.to_bits(),
            0.3_f32.to_bits(),
        ]
    );
    // The golden pins the effect order and the grain seed's domain; it moves
    // when either does.
    assert_eq!(stable_pixel_hash(&image), 0x46ee_f228_a396_a1b3);
}

fn stable_pixel_hash(image: &CpuImage) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
    for pixel in image.pixels() {
        for channel in [pixel.red(), pixel.green(), pixel.blue(), pixel.alpha()] {
            for byte in channel.to_bits().to_le_bytes() {
                hash ^= u64::from(byte);
                hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
            }
        }
    }
    hash
}

#[test]
fn vignette_then_sharpness_then_grain_order_has_a_focused_golden() {
    let pixels = (0..20)
        .map(|index| {
            let x = (index % 5) as f32;
            let y = (index / 5) as f32;
            RgbaPixel::new(0.1 + x * 0.13, 0.2 + y * 0.11, 0.8 - x * 0.07, 1.0).unwrap()
        })
        .collect();
    let mut image = CpuImage::new(5, 4, pixels).unwrap();
    let mut settings = DevelopSettings::default();
    settings.effects.vignette = -30.0;
    settings.effects.sharpness = 45.0;
    settings.effects.grain.amount = 32.0;
    settings.effects.grain.size_iso = 4000.0;
    settings.effects.grain.midtone_response = 80.0;
    let context = DevelopRenderContext::from_source_digest([0xa7; 32]);

    DevelopPipeline
        .process_with_context(&mut image, &settings, Some(&context))
        .unwrap();
    assert_eq!(stable_pixel_hash(&image), 0x1f66_a612_9526_1406);
}
