use grainroom::develop::{
    CpuImage, DevelopPipeline, DevelopRenderContext, DevelopSettings, DevelopStage, PipelineError,
    ResolvedGrainSeed, RgbaPixel,
};

fn active_grain() -> DevelopSettings {
    let mut settings = DevelopSettings::default();
    settings.effects.grain.amount = 61.0;
    settings.effects.grain.size_iso = 4000.0;
    settings.effects.grain.midtone_response = 73.0;
    settings
}

fn patterned_image(width: u32, height: u32) -> CpuImage {
    CpuImage::new(
        width,
        height,
        (0..width * height)
            .map(|index| {
                let value = index as f32 / (width * height) as f32;
                RgbaPixel::new(value * 2.0 - 0.5, 0.2 + value, 3.0 - value, 0.37).unwrap()
            })
            .collect(),
    )
    .unwrap()
}

fn render(source: &CpuImage, context: &DevelopRenderContext) -> CpuImage {
    let mut rendered = source.clone();
    DevelopPipeline
        .process_with_context(&mut rendered, &active_grain(), Some(context))
        .unwrap();
    rendered
}

#[test]
fn active_grain_without_context_is_an_exact_atomic_error() {
    let mut image = patterned_image(8, 6);
    let original = image.clone();
    let settings = active_grain();
    assert_eq!(
        DevelopPipeline.preflight(&settings),
        Err(PipelineError::MissingRenderContext(DevelopStage::Effects))
    );
    assert_eq!(
        DevelopPipeline.process(&mut image, &settings),
        Err(PipelineError::MissingRenderContext(DevelopStage::Effects))
    );
    assert_eq!(image, original);
}

#[test]
fn source_digest_identity_is_stable_and_content_sensitive() {
    let source = patterned_image(8, 6);
    let digest = [0x42; 32];
    let first = DevelopRenderContext::from_source_digest(digest);
    let same_content = DevelopRenderContext::from_source_digest(digest);
    let mut different_digest = digest;
    different_digest[7] ^= 1;
    let different_content = DevelopRenderContext::from_source_digest(different_digest);

    // The API accepts content identity only: no filename, path, mtime, or IO
    // participates, so a rename cannot affect either of the equal renders.
    assert_eq!(render(&source, &first), render(&source, &same_content));
    assert_ne!(render(&source, &first), render(&source, &different_content));
}

#[test]
fn explicit_fixed_seed_repeats_exactly_and_preserves_scene_contract() {
    let context = DevelopRenderContext::from_resolved_grain_seed(
        ResolvedGrainSeed::fixed_for_tests(0x1234_5678_9abc_def0),
    );
    let source = CpuImage::new(
        3,
        1,
        vec![
            RgbaPixel::new(-2.0, 0.5, 8.0, 0.25).unwrap(),
            RgbaPixel::new(4.0, -1.0, 0.18, 0.5).unwrap(),
            RgbaPixel::new(f32::MAX, -f32::MAX, f32::MAX, 0.75).unwrap(),
        ],
    )
    .unwrap();
    let first = render(&source, &context);
    let second = render(&source, &context);
    assert_eq!(first, second);
    assert_ne!(first, source);
    for (output, input) in first.pixels().iter().zip(source.pixels()) {
        assert_eq!(output.alpha().to_bits(), input.alpha().to_bits());
        assert!(output.red().is_finite());
        assert!(output.green().is_finite());
        assert!(output.blue().is_finite());
    }
}

#[test]
fn active_oversized_grain_maps_to_a_transactional_effects_error() {
    let width = (1_u32 << 20) + 1;
    let mut image = CpuImage::new(
        width,
        1,
        vec![RgbaPixel::new(0.18, 0.18, 0.18, 1.0).unwrap(); width as usize],
    )
    .unwrap();
    let original = image.clone();
    let context =
        DevelopRenderContext::from_resolved_grain_seed(ResolvedGrainSeed::fixed_for_tests(1));
    assert_eq!(
        DevelopPipeline.process_with_context(&mut image, &active_grain(), Some(&context),),
        Err(PipelineError::NumericFailure {
            stage: DevelopStage::Effects,
            reason: "grain extent exceeds the supported dimension",
        })
    );
    assert_eq!(image, original);
}
