use omalux::develop::{CpuImage, DevelopPipeline, DevelopRenderContext, PresetCatalog, RgbaPixel};
use omalux::io::ResourceLimits;

fn dark_edge_pattern(width: u32, height: u32) -> CpuImage {
    let mut pixels = Vec::with_capacity((width * height) as usize);
    for y in 0..height {
        for x in 0..width {
            let checker = if (x / 5 + y / 7) % 2 == 0 { 0.0 } else { 1.0 };
            let ramp = (x + 3 * y) as f32 / (width + 3 * height - 4) as f32;
            let value = 0.000_01 + 0.12 * checker + 0.88 * ramp;
            pixels.push(RgbaPixel::new(value, value * 0.37, value * 0.08, 1.0).unwrap());
        }
    }
    CpuImage::new(width, height, pixels).unwrap()
}

#[test]
fn all_28_builtin_presets_render_a_dark_edge_pattern_bounded() {
    let catalog = PresetCatalog::built_in().unwrap();
    assert_eq!(catalog.documents().len(), 28);
    let context = DevelopRenderContext::from_source_digest([0x51; 32]);
    for preset in catalog.documents() {
        let mut image = dark_edge_pattern(67, 53);
        let original = image.clone();
        let result = DevelopPipeline.process_bounded_with_context(
            &mut image,
            &preset.settings,
            Some(&context),
            &ResourceLimits::default(),
        );
        assert!(
            result.is_ok(),
            "built-in preset {} failed on finite synthetic input: {:?}",
            preset.id,
            result.unwrap_err()
        );
        assert!(
            image.validate().is_ok(),
            "{} produced an invalid image",
            preset.id
        );
        if preset.settings.is_neutral() {
            assert_eq!(image, original);
        }
    }
}
