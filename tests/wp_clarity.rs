use omalux::develop::{CpuImage, DevelopPipeline, DevelopSettings, RgbaPixel};

const LUMA: [f64; 3] = [0.262_700_2, 0.677_998_1, 0.059_301_7];

#[test]
fn zero_is_bit_exact_and_nonzero_clarity_is_supported() {
    let source = patterned_image(31, 19);
    let mut neutral = source.clone();
    DevelopPipeline
        .process(&mut neutral, &DevelopSettings::default())
        .unwrap();
    assert_eq!(neutral, source);

    let mut active = DevelopSettings::default();
    active.basics.clarity = 50.0;
    assert_eq!(DevelopPipeline.preflight(&active), Ok(()));
    let mut rendered = source.clone();
    DevelopPipeline.process(&mut rendered, &active).unwrap();
    assert_ne!(rendered, source);
}

#[test]
fn signed_hdr_flat_fields_and_alpha_are_preserved() {
    for rgb in [[0.18, 0.18, 0.18], [-4.0, 0.5, 8.0], [12.0, -3.0, 0.25]] {
        let source = CpuImage::new(
            23,
            17,
            vec![RgbaPixel::new(rgb[0], rgb[1], rgb[2], 0.37).unwrap(); 23 * 17],
        )
        .unwrap();
        for amount in [-100.0, 100.0] {
            let rendered = render(source.clone(), amount);
            for (before, after) in source.pixels().iter().zip(rendered.pixels()) {
                assert_eq!(after.alpha().to_bits(), before.alpha().to_bits());
                assert_close(after.red(), before.red(), 2.0e-5);
                assert_close(after.green(), before.green(), 2.0e-5);
                assert_close(after.blue(), before.blue(), 2.0e-5);
            }
        }
    }
}

#[test]
fn positive_and_negative_amounts_change_local_texture_monotonically() {
    let source = grayscale_image(65, 33, |x, _| {
        0.18 + 0.025 * (std::f64::consts::TAU * x as f64 / 8.0).sin()
    });
    let levels =
        [-100.0, -50.0, 0.0, 50.0, 100.0].map(|amount| local_rms(&render(source.clone(), amount)));
    assert!(
        levels.windows(2).all(|pair| pair[0] < pair[1]),
        "{levels:?}"
    );
}

#[test]
fn hard_step_halo_is_bounded_and_confined_to_the_declared_support() {
    let source = grayscale_image(81, 25, |x, _| if x < 40 { 0.08 } else { 0.8 });
    let rendered = render(source, 100.0);
    let row = 12 * 81;
    assert_close(rendered.pixels()[row + 4].red(), 0.08, 2.0e-5);
    assert_close(rendered.pixels()[row + 76].red(), 0.8, 2.0e-5);
    for y in 0..25 {
        for x in 0..81 {
            let value = rendered.pixels()[y * 81 + x].red();
            assert!((0.05..=0.90).contains(&value), "{value}");
            if x <= 22 {
                assert_close(value, 0.08, 2.0e-5);
            } else if x >= 57 {
                assert_close(value, 0.8, 2.0e-5);
            }
        }
    }
}

#[test]
fn negative_clarity_suppresses_deterministic_noise() {
    let source = grayscale_image(47, 29, |x, y| {
        let hash = (x * 37 + y * 101 + x * y * 13) % 29;
        0.18 + (hash as f64 - 14.0) * 0.0015
    });
    let first = render(source.clone(), -80.0);
    let second = render(source.clone(), -80.0);
    assert_eq!(first, second);
    assert!(local_rms(&first) < local_rms(&source));
}

#[test]
fn mixed_sign_and_f32_extreme_inputs_stay_finite() {
    let pixels = (0..19 * 17)
        .map(|index| match index % 3 {
            0 => RgbaPixel::new(f32::MAX, -f32::MAX, 0.0, 1.0).unwrap(),
            1 => RgbaPixel::new(-f32::MAX, f32::MAX, f32::MAX, 0.5).unwrap(),
            _ => RgbaPixel::new(-12.0, 0.25, 64.0, 0.0).unwrap(),
        })
        .collect();
    let source = CpuImage::new(19, 17, pixels).unwrap();
    for amount in [-100.0, 100.0] {
        let rendered = render(source.clone(), amount);
        assert!(rendered.pixels().iter().all(|pixel| {
            pixel.red().is_finite() && pixel.green().is_finite() && pixel.blue().is_finite()
        }));
        for (before, after) in source.pixels().iter().zip(rendered.pixels()) {
            assert_eq!(after.alpha().to_bits(), before.alpha().to_bits());
        }
    }
}

#[test]
fn clarity_changes_luma_neutrally_and_preserves_opponent_differences() {
    let source = patterned_image(35, 19);
    let rendered = render(source.clone(), 85.0);
    for (before, after) in source.pixels().iter().zip(rendered.pixels()) {
        assert_close(
            after.red() - after.green(),
            before.red() - before.green(),
            3.0e-7,
        );
        assert_close(
            after.green() - after.blue(),
            before.green() - before.blue(),
            3.0e-7,
        );
    }
}

#[test]
fn clarity_is_ordered_after_contrast_and_before_saturation() {
    let source = patterned_image(29, 21);
    let mut combined_settings = DevelopSettings::default();
    combined_settings.basics.highlights = -15.0;
    combined_settings.basics.contrast = 25.0;
    combined_settings.basics.clarity = 60.0;
    combined_settings.basics.saturation = 35.0;
    let mut combined = source.clone();
    DevelopPipeline
        .process(&mut combined, &combined_settings)
        .unwrap();

    let mut pre_clarity = DevelopSettings::default();
    pre_clarity.basics.highlights = -15.0;
    pre_clarity.basics.contrast = 25.0;
    let mut clarity = DevelopSettings::default();
    clarity.basics.clarity = 60.0;
    let mut saturation = DevelopSettings::default();
    saturation.basics.saturation = 35.0;
    let mut sequential = source;
    DevelopPipeline
        .process(&mut sequential, &pre_clarity)
        .unwrap();
    DevelopPipeline.process(&mut sequential, &clarity).unwrap();
    DevelopPipeline
        .process(&mut sequential, &saturation)
        .unwrap();
    assert_eq!(combined, sequential);
}

fn render(mut image: CpuImage, amount: f32) -> CpuImage {
    let mut settings = DevelopSettings::default();
    settings.basics.clarity = amount;
    DevelopPipeline.process(&mut image, &settings).unwrap();
    image
}

fn grayscale_image(width: usize, height: usize, value: impl Fn(usize, usize) -> f64) -> CpuImage {
    CpuImage::new(
        width as u32,
        height as u32,
        (0..height)
            .flat_map(|y| {
                let value = &value;
                (0..width).map(move |x| {
                    let gray = value(x, y) as f32;
                    RgbaPixel::new(gray, gray, gray, 0.8).unwrap()
                })
            })
            .collect(),
    )
    .unwrap()
}

fn patterned_image(width: usize, height: usize) -> CpuImage {
    CpuImage::new(
        width as u32,
        height as u32,
        (0..width * height)
            .map(|index| {
                let x = index % width;
                let y = index / width;
                let wave = (std::f64::consts::TAU * x as f64 / 9.0).sin() * 0.04;
                RgbaPixel::new(
                    (0.20 + wave) as f32,
                    (0.12 + 0.5 * wave + y as f64 * 0.001) as f32,
                    (1.5 - wave) as f32,
                    0.25 + (index % 4) as f32 * 0.2,
                )
                .unwrap()
            })
            .collect(),
    )
    .unwrap()
}

fn local_rms(image: &CpuImage) -> f64 {
    let values = image
        .pixels()
        .iter()
        .map(|pixel| {
            LUMA[0] * f64::from(pixel.red())
                + LUMA[1] * f64::from(pixel.green())
                + LUMA[2] * f64::from(pixel.blue())
        })
        .collect::<Vec<_>>();
    let mean = values.iter().sum::<f64>() / values.len() as f64;
    (values
        .iter()
        .map(|value| (value - mean).powi(2))
        .sum::<f64>()
        / values.len() as f64)
        .sqrt()
}

fn assert_close(actual: f32, expected: f32, tolerance: f32) {
    assert!(
        (actual - expected).abs() <= tolerance,
        "expected {expected}, got {actual}"
    );
}
