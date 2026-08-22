use grainroom::develop::{
    CpuImage, CurvePoint, DevelopPipeline, DevelopSettings, RgbaPixel, ToneCurve,
};

const LUMA: [f64; 3] = [0.262_700_2, 0.677_998_1, 0.059_301_7];

#[test]
fn neutral_curves_are_bit_exact_for_negative_and_hdr_pixels() {
    let mut image = CpuImage::new(
        2,
        1,
        vec![pixel([-0.5, 0.25, 8.0]), pixel([2.0, -1.0, 0.0])],
    )
    .unwrap();
    let original = image.clone();
    DevelopPipeline
        .process(&mut image, &DevelopSettings::default())
        .unwrap();
    assert_eq!(image, original);
}

#[test]
fn pchip_matches_normative_golden_values() {
    let curve = golden_curve();
    for (input, expected) in [(0.25, 0.078_125), (0.75, 0.546_875)] {
        let output = render([input; 3], |settings| {
            settings.tone_curves.red = curve.clone();
            settings.tone_curves.green = curve.clone();
            settings.tone_curves.blue = curve.clone();
        });
        assert_rgb_close(output, expected, 2.0e-6);
    }
}

#[test]
fn endpoint_slopes_linearly_extrapolate_unbounded_values() {
    let curve = golden_curve();
    let below = render([-0.25; 3], |settings| {
        settings.tone_curves.red = curve.clone();
        settings.tone_curves.green = curve.clone();
        settings.tone_curves.blue = curve.clone();
    });
    let above = render([1.25; 3], |settings| {
        settings.tone_curves.red = curve.clone();
        settings.tone_curves.green = curve.clone();
        settings.tone_curves.blue = curve.clone();
    });
    assert_rgb_close(below, 0.0, 1.0e-7);
    assert_rgb_close(above, 1.5, 2.0e-6);
}

#[test]
fn master_curve_changes_luminance_without_changing_rgb_ratios() {
    let input = [0.4, 0.2, 0.1];
    let input_luma = luma(input);
    let output = render(input, |settings| {
        settings.tone_curves.master = golden_curve()
    });
    let expected_luma = golden_pchip(input_luma);
    assert_close(luma(output), expected_luma, 3.0e-6);
    assert_close(output[0] / output[1], input[0] / input[1], 2.0e-6);
    assert_close(output[1] / output[2], input[1] / input[2], 2.0e-6);
}

#[test]
fn master_curve_lifts_exact_black_and_preserves_alpha() {
    let mut settings = DevelopSettings::default();
    settings.tone_curves.master = lifted_curve();
    let mut image = CpuImage::new(1, 1, vec![pixel([0.0; 3])]).unwrap();
    DevelopPipeline.process(&mut image, &settings).unwrap();
    let output = image.pixels()[0];
    assert_rgb_close(
        [
            f64::from(output.red()),
            f64::from(output.green()),
            f64::from(output.blue()),
        ],
        0.2,
        2.0e-7,
    );
    assert_eq!(output.alpha().to_bits(), 0.75_f32.to_bits());
}

#[test]
fn master_curve_handles_chromatic_near_zero_luminance() {
    let input = [1.0, -LUMA[0] / LUMA[1], 0.0];
    let output = render(input, |settings| {
        settings.tone_curves.master = lifted_curve()
    });
    assert_close(luma(output), 0.2, 3.0e-7);
    assert!(output.into_iter().all(f64::is_finite));
}

#[test]
fn master_curve_extrapolates_negative_luminance() {
    let output = render([-0.25; 3], |settings| {
        settings.tone_curves.master = lifted_curve()
    });
    assert_rgb_close(output, 0.0, 2.0e-7);
}

#[test]
fn master_is_applied_before_individual_rgb_curves() {
    let square = ToneCurve {
        points: vec![
            CurvePoint { x: 0.0, y: 0.0 },
            CurvePoint { x: 0.5, y: 0.25 },
            CurvePoint { x: 1.0, y: 1.0 },
        ],
    };
    let output = render([0.5; 3], |settings| {
        settings.tone_curves.master = square.clone();
        settings.tone_curves.red = square;
    });
    assert_close(output[0], 0.078_125, 2.0e-6);
    assert_close(output[1], 0.25, 2.0e-6);
    assert_close(output[2], 0.25, 2.0e-6);
}

#[test]
fn generated_curve_is_monotone() {
    let curve = ToneCurve {
        points: vec![
            CurvePoint { x: 0.0, y: 0.0 },
            CurvePoint { x: 0.2, y: 0.05 },
            CurvePoint { x: 0.4, y: 0.05 },
            CurvePoint { x: 0.7, y: 0.8 },
            CurvePoint { x: 1.0, y: 1.0 },
        ],
    };
    let mut previous = f64::NEG_INFINITY;
    for index in 0..=1000 {
        let input = index as f64 / 1000.0;
        let output = render([input; 3], |settings| {
            settings.tone_curves.red = curve.clone();
            settings.tone_curves.green = curve.clone();
            settings.tone_curves.blue = curve.clone();
        })[0];
        assert!(output >= previous - 1.0e-7, "curve fell at {input}");
        previous = output;
    }
}

#[test]
fn tightly_spaced_curve_nodes_are_evaluated_exactly() {
    let low_x = 0.5_f32;
    let high_x = f32::from_bits(low_x.to_bits() + 2);
    let curve = ToneCurve {
        points: vec![
            CurvePoint { x: 0.0, y: 0.0 },
            CurvePoint { x: low_x, y: 0.0 },
            CurvePoint { x: high_x, y: 1.0 },
            CurvePoint { x: 1.0, y: 1.0 },
        ],
    };
    for (input, expected) in [(low_x, 0.0_f32), (high_x, 1.0_f32)] {
        let output = render([f64::from(input); 3], |settings| {
            settings.tone_curves.red = curve.clone();
        });
        assert_eq!((output[0] as f32).to_bits(), expected.to_bits());
    }
}

#[test]
fn maximum_density_curve_processes_a_large_scanline_monotonically() {
    let points = (0..32)
        .map(|index| {
            let value = index as f32 / 31.0;
            CurvePoint {
                x: value,
                y: value * value,
            }
        })
        .collect();
    let mut settings = DevelopSettings::default();
    settings.tone_curves.red = ToneCurve { points };
    let pixels = (0..65_536)
        .map(|index| pixel([index as f64 / 65_535.0, 0.0, 0.0]))
        .collect();
    let mut image = CpuImage::new(65_536, 1, pixels).unwrap();
    DevelopPipeline.process(&mut image, &settings).unwrap();
    for pair in image.pixels().windows(2) {
        assert!(pair[1].red() >= pair[0].red());
    }
}

#[test]
fn descending_y_is_rejected_with_a_precise_path() {
    let mut settings = DevelopSettings::default();
    settings.tone_curves.red.points = vec![
        CurvePoint { x: 0.0, y: 0.0 },
        CurvePoint { x: 0.5, y: 0.8 },
        CurvePoint { x: 1.0, y: 0.7 },
    ];
    let error = settings.validate().unwrap_err();
    assert_eq!(error.path(), "tone_curves.red.points[2].y");
    assert!(error.message().contains("nondecreasing"));
}

#[test]
fn curve_processing_preserves_alpha() {
    let curve = golden_curve();
    let mut settings = DevelopSettings::default();
    settings.tone_curves.red = curve;
    let mut image = CpuImage::new(1, 1, vec![pixel([0.25, 0.5, 2.0])]).unwrap();
    DevelopPipeline.process(&mut image, &settings).unwrap();
    assert_eq!(image.pixels()[0].alpha().to_bits(), 0.75_f32.to_bits());
}

fn golden_curve() -> ToneCurve {
    ToneCurve {
        points: vec![
            CurvePoint { x: 0.0, y: 0.0 },
            CurvePoint { x: 0.5, y: 0.25 },
            CurvePoint { x: 1.0, y: 1.0 },
        ],
    }
}

fn lifted_curve() -> ToneCurve {
    ToneCurve {
        points: vec![
            CurvePoint { x: 0.0, y: 0.2 },
            CurvePoint { x: 0.5, y: 0.6 },
            CurvePoint { x: 1.0, y: 1.0 },
        ],
    }
}

fn golden_pchip(x: f64) -> f64 {
    if x <= 0.5 {
        let t = x / 0.5;
        -0.125 * t * t * t + 0.375 * t * t
    } else {
        let t = (x - 0.5) / 0.5;
        -0.125 * t * t * t + 0.5 * t * t + 0.375 * t + 0.25
    }
}

fn render(rgb: [f64; 3], configure: impl FnOnce(&mut DevelopSettings)) -> [f64; 3] {
    let mut settings = DevelopSettings::default();
    configure(&mut settings);
    let mut image = CpuImage::new(1, 1, vec![pixel(rgb)]).unwrap();
    DevelopPipeline.process(&mut image, &settings).unwrap();
    let pixel = image.pixels()[0];
    [
        f64::from(pixel.red()),
        f64::from(pixel.green()),
        f64::from(pixel.blue()),
    ]
}

fn pixel(rgb: [f64; 3]) -> RgbaPixel {
    RgbaPixel::new(rgb[0] as f32, rgb[1] as f32, rgb[2] as f32, 0.75).unwrap()
}

fn luma(rgb: [f64; 3]) -> f64 {
    rgb[0] * LUMA[0] + rgb[1] * LUMA[1] + rgb[2] * LUMA[2]
}

fn assert_rgb_close(actual: [f64; 3], expected: f64, tolerance: f64) {
    for channel in actual {
        assert_close(channel, expected, tolerance);
    }
}

fn assert_close(actual: f64, expected: f64, tolerance: f64) {
    assert!(
        (actual - expected).abs() <= tolerance,
        "expected {expected}, got {actual}"
    );
}
