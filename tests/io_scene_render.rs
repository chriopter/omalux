use grainroom::{
    develop::RgbaPixel,
    io::{
        ResourceLimits, SdrRangePolicy, SignalRelation,
        color::{
            SceneRenderError, SceneToDisplayTransform, WorkingToSrgbTransform,
            estimate_scene_render_working_set,
        },
    },
};

fn pixel(red: f32, green: f32, blue: f32, alpha: f32) -> RgbaPixel {
    RgbaPixel::new(red, green, blue, alpha).unwrap()
}

fn blank(count: usize) -> Vec<RgbaPixel> {
    vec![pixel(9.0, -3.0, 2.0, 0.5); count]
}

fn assert_close(actual: f32, expected: f32, tolerance: f32) {
    assert!(
        (actual - expected).abs() <= tolerance,
        "expected {expected}, got {actual} (tolerance {tolerance})"
    );
}

#[test]
fn neutral_log_logistic_goldens_are_pinned() {
    let source = [
        pixel(0.0, 0.0, 0.0, 1.0),
        pixel(0.01, 0.01, 0.01, 1.0),
        pixel(0.18, 0.18, 0.18, 1.0),
        pixel(1.0, 1.0, 1.0, 1.0),
        pixel(4.0, 4.0, 4.0, 1.0),
    ];
    let mut output = blank(source.len());
    SceneToDisplayTransform::new()
        .transform_scanline(
            &source,
            &mut output,
            SignalRelation::SceneRelatedRaw,
            &ResourceLimits::default(),
        )
        .unwrap();
    let expected = [0.0, 0.001_609_888, 0.18, 0.801_994_9, 0.977_146];
    for (pixel, expected) in output.iter().zip(expected) {
        for actual in [pixel.red(), pixel.green(), pixel.blue()] {
            assert_close(actual, expected, 2.0e-6);
        }
    }
}

#[test]
fn signed_hdr_extremes_are_finite_bounded_and_alpha_exact() {
    let alphas = [f32::from_bits(1), -0.0, 0.5, 1.0];
    let source = [
        pixel(-4.0, -2.0, -1.0, alphas[0]),
        pixel(f32::MAX, -f32::MAX, f32::MAX, alphas[1]),
        pixel(64.0, 0.01, -2.0, alphas[2]),
        pixel(-8.0, 16.0, 2.0, alphas[3]),
    ];
    let mut output = blank(source.len());
    let report = SceneToDisplayTransform::new()
        .transform_scanline(
            &source,
            &mut output,
            SignalRelation::SceneRelatedRaw,
            &ResourceLimits::default(),
        )
        .unwrap();
    assert_eq!(
        report.input_signal_relation,
        SignalRelation::SceneRelatedRaw
    );
    assert_eq!(
        report.output_signal_relation,
        SignalRelation::LinearizedDisplayReferred
    );
    assert_eq!(report.nonpositive_luminance_pixels, 2);
    assert!(report.gamut_compressed_pixels >= 2);
    for (output, alpha) in output.iter().zip(alphas) {
        for sample in [output.red(), output.green(), output.blue()] {
            assert!(sample.is_finite());
            assert!((0.0..=1.0).contains(&sample));
        }
        assert_eq!(output.alpha().to_bits(), alpha.to_bits());
    }
}

#[test]
fn rendered_scene_is_accepted_by_the_existing_lcms_srgb_boundary() {
    let source = [
        pixel(0.18, 0.18, 0.18, 0.125),
        pixel(3.0, 0.1, -0.05, 0.5),
        pixel(0.0, 2.0, 0.2, 1.0),
    ];
    let limits = ResourceLimits::default();
    let mut rendered = blank(source.len());
    let report = SceneToDisplayTransform::new()
        .transform_scanline(
            &source,
            &mut rendered,
            SignalRelation::SceneRelatedRaw,
            &limits,
        )
        .unwrap();
    let mut encoded = vec![[0.0; 4]; source.len()];
    let output_report = WorkingToSrgbTransform::new(&limits)
        .unwrap()
        .transform_scanline(
            &rendered,
            &mut encoded,
            report.output_signal_relation,
            SdrRangePolicy::Reject,
            &limits,
        )
        .unwrap();
    assert_eq!(output_report.clipped_samples, 0);
    for (encoded, source) in encoded.iter().zip(source) {
        assert!(
            encoded[..3]
                .iter()
                .all(|sample| (0.0..=1.0).contains(sample))
        );
        assert_eq!(encoded[3].to_bits(), source.alpha().to_bits());
    }
}

#[test]
fn errors_are_transactional_and_relation_is_fail_closed() {
    let source = [pixel(0.18, 0.18, 0.18, 1.0), pixel(2.0, 1.0, 0.0, 0.25)];
    let original = blank(source.len());
    let renderer = SceneToDisplayTransform::new();

    let mut wrong_relation = original.clone();
    assert!(matches!(
        renderer.transform_scanline(
            &source,
            &mut wrong_relation,
            SignalRelation::LinearizedDisplayReferred,
            &ResourceLimits::default(),
        ),
        Err(SceneRenderError::InvalidSignalRelation { .. })
    ));
    assert_eq!(wrong_relation, original);

    let mut wrong_length = original.clone();
    assert!(matches!(
        renderer.transform_scanline(
            &source[..1],
            &mut wrong_length,
            SignalRelation::SceneRelatedRaw,
            &ResourceLimits::default(),
        ),
        Err(SceneRenderError::LengthMismatch { .. })
    ));
    assert_eq!(wrong_length, original);
}

#[test]
fn scanline_partitioning_is_deterministic_and_memory_is_linear() {
    let source: Vec<_> = (0..257)
        .map(|index| {
            let x = index as f32 / 256.0;
            pixel(8.0 * x - 0.5, 2.0 - x, x * x, (index % 17) as f32 / 16.0)
        })
        .collect();
    let renderer = SceneToDisplayTransform::new();
    let limits = ResourceLimits::default();
    let mut whole = blank(source.len());
    renderer
        .transform_scanline(
            &source,
            &mut whole,
            SignalRelation::SceneRelatedRaw,
            &limits,
        )
        .unwrap();

    for chunk_size in [1, 7, 64, 128] {
        let mut partitioned = blank(source.len());
        for (source, destination) in source
            .chunks(chunk_size)
            .zip(partitioned.chunks_mut(chunk_size))
        {
            renderer
                .transform_scanline(
                    source,
                    destination,
                    SignalRelation::SceneRelatedRaw,
                    &limits,
                )
                .unwrap();
        }
        assert_eq!(partitioned, whole);
    }

    let estimate = estimate_scene_render_working_set(257, &limits).unwrap();
    assert_eq!(estimate.scratch_bytes, 257 * 16);
}

#[test]
fn broad_signed_hdr_corpus_stays_inside_the_lcms_srgb_contract() {
    let mut state = 0x6a09_e667_f3bc_c909_u64;
    let mut next = || {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        let unit = (state >> 40) as f32 / ((1_u32 << 24) - 1) as f32;
        unit * 256.0 - 64.0
    };
    let source: Vec<_> = (0..4096)
        .map(|index| pixel(next(), next(), next(), (index % 257) as f32 / 256.0))
        .collect();
    let limits = ResourceLimits::default();
    let mut rendered = blank(source.len());
    let report = SceneToDisplayTransform::new()
        .transform_scanline(
            &source,
            &mut rendered,
            SignalRelation::SceneRelatedRaw,
            &limits,
        )
        .unwrap();
    let mut encoded = vec![[0.0; 4]; source.len()];
    WorkingToSrgbTransform::new(&limits)
        .unwrap()
        .transform_scanline(
            &rendered,
            &mut encoded,
            report.output_signal_relation,
            SdrRangePolicy::Reject,
            &limits,
        )
        .unwrap();
    assert!(
        encoded
            .iter()
            .all(|pixel| pixel[..3].iter().all(|sample| (0.0..=1.0).contains(sample)))
    );
}
