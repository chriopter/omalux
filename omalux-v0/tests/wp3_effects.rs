use omalux::develop::{
    CpuImage, DevelopPipeline, DevelopSettings, DevelopStage, PipelineError, RgbaPixel,
};

#[test]
fn zero_grain_is_bit_exact_and_active_grain_requires_context() {
    let original = patterned_image(9, 7);
    let mut rendered = original.clone();
    let mut settings = DevelopSettings::default();
    settings.effects.grain.size_iso = 6400.0;
    settings.effects.grain.midtone_response = 0.0;
    DevelopPipeline.process(&mut rendered, &settings).unwrap();
    assert_eq!(rendered, original);

    settings.effects.grain.amount = 1.0;
    assert_eq!(
        DevelopPipeline.process(&mut rendered, &settings),
        Err(PipelineError::MissingRenderContext(DevelopStage::Effects))
    );
    assert_eq!(rendered, original);
}

#[test]
fn bloom_spreads_highlights_additively_and_preserves_alpha() {
    let mut image = impulse_image(17, 17, [5.0, 3.0, 1.0], 0.4);
    let alpha = alphas(&image);
    let mut settings = DevelopSettings::default();
    settings.effects.bloom = 70.0;
    DevelopPipeline.process(&mut image, &settings).unwrap();

    assert_eq!(alphas(&image), alpha);
    let center = rgb(&image, 8, 8);
    let neighbor = rgb(&image, 9, 8);
    assert!(center[0] > 5.0);
    assert!(neighbor[0] > 0.0);
    assert!(neighbor[1] > 0.0);
    assert!(neighbor[2] > 0.0);
}

#[test]
fn halation_is_a_distinct_warm_outer_halo() {
    let mut image = impulse_image(33, 33, [6.0, 6.0, 6.0], 1.0);
    let mut settings = DevelopSettings::default();
    settings.effects.halation = 100.0;
    DevelopPipeline.process(&mut image, &settings).unwrap();

    let halo = rgb(&image, 19, 16);
    assert!(halo[0] > 0.0);
    assert!(halo[0] > halo[1]);
    assert!(halo[1] > halo[2]);
    assert_eq!(rgb(&image, 16, 16), [6.0, 6.0, 6.0]);
}

#[test]
fn halation_uses_luminance_energy_for_red_green_and_blue_highlights() {
    for impulse in [
        [6.0 / 0.2627, 0.0, 0.0],
        [0.0, 6.0 / 0.6780, 0.0],
        [0.0, 0.0, 6.0 / 0.0593],
    ] {
        let mut image = impulse_image(65, 65, impulse, 1.0);
        let settings = settings(0.0, 100.0, 0.0, 0.0, 0.0);
        DevelopPipeline.process(&mut image, &settings).unwrap();
        let halo = rgb(&image, 37, 32);
        assert!(halo[0] > 0.0, "impulse={impulse:?}");
        assert!(halo[0] > halo[1] && halo[1] > halo[2]);
    }
}

#[test]
fn fade_lifts_black_and_compresses_unbounded_scene_values() {
    let pixels = vec![
        RgbaPixel::new(0.0, 0.0, 0.0, 0.25).unwrap(),
        RgbaPixel::new(-2.0, 4.0, 20.0, 0.75).unwrap(),
    ];
    let mut image = CpuImage::new(2, 1, pixels).unwrap();
    let mut settings = DevelopSettings::default();
    settings.effects.fade = 100.0;
    DevelopPipeline.process(&mut image, &settings).unwrap();

    // Full fade puts the print floor at 4% and its ceiling at 94% of the
    // display range; scene black therefore lands on the linearized 4% floor.
    let floor = rgb(&image, 0, 0);
    let expected_floor = ((0.04_f32 + 0.055) / 1.055).powf(2.4);
    for channel in floor {
        assert!((channel - expected_floor).abs() < 1.0e-6, "floor {channel}");
    }
    // Unbounded scene values compress under the ceiling instead of clipping.
    let high = rgb(&image, 1, 0);
    let ceiling = ((0.94_f32 + 0.055) / 1.055).powf(2.4);
    assert!(high[0] > -2.0);
    assert!(high[1] < ceiling);
    assert!(high[2] < ceiling);
    assert!(high[2] > high[1]);
    assert_eq!(alphas(&image), vec![0.25, 0.75]);
}

#[test]
fn vignette_uses_symmetric_full_frame_coordinates() {
    let pixels = vec![RgbaPixel::new(1.0, 1.0, 1.0, 1.0).unwrap(); 21 * 21];
    let mut image = CpuImage::new(21, 21, pixels).unwrap();
    let mut settings = DevelopSettings::default();
    settings.effects.vignette = 100.0;
    DevelopPipeline.process(&mut image, &settings).unwrap();

    let center = rgb(&image, 10, 10)[0];
    let corners = [
        rgb(&image, 0, 0)[0],
        rgb(&image, 20, 0)[0],
        rgb(&image, 0, 20)[0],
        rgb(&image, 20, 20)[0],
    ];
    assert_eq!(center, 1.0);
    assert!(corners[0] < center);
    assert!(corners.iter().all(|corner| *corner == corners[0]));
}

#[test]
fn vignette_is_centered_and_symmetric_on_a_nonsquare_frame() {
    let pixels = vec![RgbaPixel::new(1.0, 1.0, 1.0, 1.0).unwrap(); 31 * 11];
    let mut image = CpuImage::new(31, 11, pixels).unwrap();
    let settings = settings(0.0, 0.0, 0.0, 80.0, 0.0);
    DevelopPipeline.process(&mut image, &settings).unwrap();
    assert_eq!(rgb(&image, 15, 5)[0], 1.0);
    assert_eq!(rgb(&image, 0, 5)[0], rgb(&image, 30, 5)[0]);
    assert_eq!(rgb(&image, 15, 0)[0], rgb(&image, 15, 10)[0]);
    assert_eq!(rgb(&image, 0, 0)[0], rgb(&image, 30, 10)[0]);
}

#[test]
fn optical_profiles_are_normalized_across_one_two_and_four_x_frames() {
    let bloom_profiles = [65_u32, 129, 257]
        .map(|size| normalized_bloom_profile(size, 100.0, &[0.0, 0.015, 0.03, 0.06]));
    for profile in bloom_profiles.iter().skip(1) {
        for (reference, candidate) in bloom_profiles[0].iter().zip(profile) {
            assert!((reference - candidate).abs() < 0.16, "{bloom_profiles:?}");
        }
    }

    let narrow = normalized_bloom_profile(257, 10.0, &[0.0, 0.02, 0.05, 0.08]);
    let broad = normalized_bloom_profile(257, 100.0, &[0.0, 0.02, 0.05, 0.08]);
    assert!(broad[2] > narrow[2]);
}

#[test]
fn thresholded_luma_usm_ignores_constant_and_enhances_an_edge() {
    let constant_pixels = vec![RgbaPixel::new(0.4, 0.4, 0.4, 0.8).unwrap(); 11 * 9];
    let mut constant = CpuImage::new(11, 9, constant_pixels).unwrap();
    let original = constant.clone();
    let mut settings = DevelopSettings::default();
    settings.effects.sharpness = 100.0;
    DevelopPipeline.process(&mut constant, &settings).unwrap();
    assert_eq!(constant, original);

    let mut pixels = Vec::new();
    for _y in 0..9 {
        for x in 0..15 {
            let value = if x < 7 { 0.1 } else { 0.9 };
            pixels.push(RgbaPixel::new(value, value, value, 1.0).unwrap());
        }
    }
    let mut edge = CpuImage::new(15, 9, pixels).unwrap();
    DevelopPipeline.process(&mut edge, &settings).unwrap();
    assert!(rgb(&edge, 6, 4)[0] < 0.1);
    assert!(rgb(&edge, 7, 4)[0] > 0.9);
}

#[test]
fn combined_effect_order_matches_sequential_pipeline_and_is_deterministic() {
    let original = patterned_image(19, 15);
    let combined_settings = settings(35.0, 28.0, 22.0, 18.0, 40.0);
    let mut first = original.clone();
    let mut second = original.clone();
    DevelopPipeline
        .process(&mut first, &combined_settings)
        .unwrap();
    DevelopPipeline
        .process(&mut second, &combined_settings)
        .unwrap();
    assert_eq!(first, second);

    let mut sequential = original;
    // Chain order: bloom, halation, vignette, sharpness, then fade, which
    // bounds the finished image and therefore runs last.
    for individual in [
        settings(35.0, 0.0, 0.0, 0.0, 0.0),
        settings(0.0, 28.0, 0.0, 0.0, 0.0),
        settings(0.0, 0.0, 0.0, 18.0, 0.0),
        settings(0.0, 0.0, 0.0, 0.0, 40.0),
        settings(0.0, 0.0, 22.0, 0.0, 0.0),
    ] {
        DevelopPipeline
            .process(&mut sequential, &individual)
            .unwrap();
    }
    assert_eq!(first, sequential);
}

#[test]
fn extreme_finite_rgb_stays_finite_and_full_frame_diffusion_has_no_tile_seam() {
    let mut image = impulse_image(65, 17, [f32::MAX, f32::MAX, f32::MAX], 1.0);
    let combined = settings(100.0, 100.0, 100.0, -100.0, 100.0);
    DevelopPipeline.process(&mut image, &combined).unwrap();
    assert!(image.pixels().iter().all(|pixel| {
        pixel.red().is_finite() && pixel.green().is_finite() && pixel.blue().is_finite()
    }));

    let mut seam_probe = impulse_image(65, 17, [8.0, 8.0, 8.0], 1.0);
    let bloom_only = settings(100.0, 0.0, 0.0, 0.0, 0.0);
    DevelopPipeline
        .process(&mut seam_probe, &bloom_only)
        .unwrap();
    let row = (0..65)
        .map(|x| rgb(&seam_probe, x, 8)[0])
        .collect::<Vec<_>>();
    for boundary in [16, 48] {
        let seam_jump = (row[boundary] - row[boundary - 1]).abs();
        let neighboring_jump = (row[boundary - 1] - row[boundary - 2])
            .abs()
            .max((row[boundary + 1] - row[boundary]).abs());
        assert!(seam_jump <= neighboring_jump * 2.0 + 1e-6);
    }
}

fn settings(
    bloom: f32,
    halation: f32,
    fade: f32,
    vignette: f32,
    sharpness: f32,
) -> DevelopSettings {
    let mut settings = DevelopSettings::default();
    settings.effects.bloom = bloom;
    settings.effects.halation = halation;
    settings.effects.fade = fade;
    settings.effects.vignette = vignette;
    settings.effects.sharpness = sharpness;
    settings
}

fn patterned_image(width: u32, height: u32) -> CpuImage {
    let mut pixels = Vec::with_capacity((width * height) as usize);
    for y in 0..height {
        for x in 0..width {
            let base = (x + y * width) as f32 / (width * height) as f32;
            pixels.push(RgbaPixel::new(base * 3.0 - 0.4, base, 1.4 - base, 0.6).unwrap());
        }
    }
    CpuImage::new(width, height, pixels).unwrap()
}

fn impulse_image(width: u32, height: u32, value: [f32; 3], alpha: f32) -> CpuImage {
    let mut pixels = vec![RgbaPixel::new(0.0, 0.0, 0.0, alpha).unwrap(); (width * height) as usize];
    let center = (height as usize / 2) * width as usize + width as usize / 2;
    pixels[center] = RgbaPixel::new(value[0], value[1], value[2], alpha).unwrap();
    CpuImage::new(width, height, pixels).unwrap()
}

fn rgb(image: &CpuImage, x: usize, y: usize) -> [f32; 3] {
    let pixel = image.pixels()[y * image.width() as usize + x];
    [pixel.red(), pixel.green(), pixel.blue()]
}

fn alphas(image: &CpuImage) -> Vec<f32> {
    image.pixels().iter().map(|pixel| pixel.alpha()).collect()
}

fn normalized_bloom_profile(size: u32, amount: f32, offsets: &[f64]) -> Vec<f32> {
    let mut image = impulse_image(size, size, [8.0, 8.0, 8.0], 1.0);
    let settings = settings(amount, 0.0, 0.0, 0.0, 0.0);
    DevelopPipeline.process(&mut image, &settings).unwrap();
    let center = size as usize / 2;
    let peak = rgb(&image, center, center)[0] - 8.0;
    offsets
        .iter()
        .map(|offset| {
            let distance = (*offset * size as f64).round() as usize;
            let original = if distance == 0 { 8.0 } else { 0.0 };
            (rgb(&image, center + distance, center)[0] - original) / peak
        })
        .collect()
}
