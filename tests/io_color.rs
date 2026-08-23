use grainroom::{
    develop::RgbaPixel,
    io::{
        ResourceLimits, SdrRangePolicy, SignalRelation,
        color::{
            ColorError, ColorWorkingSetProfile, RasterChannel, RasterToWorkingTransform,
            WorkingToSrgbTransform, estimate_color_working_set, linear_rec2020_profile,
            srgb_profile,
        },
    },
};

fn blank(count: usize) -> Vec<RgbaPixel> {
    vec![RgbaPixel::new(0.0, 0.0, 0.0, 1.0).unwrap(); count]
}

fn assert_close(actual: f32, expected: f32, tolerance: f32) {
    assert!(
        (actual - expected).abs() <= tolerance,
        "expected {expected}, got {actual} (tol {tolerance})"
    );
}

#[test]
fn srgb_transfer_and_primary_goldens_land_in_linear_rec2020() {
    let source = [
        [0.0, 0.0, 0.0, 1.0],
        [0.04045, 0.04045, 0.04045, 1.0],
        [0.5, 0.5, 0.5, 1.0],
        [1.0, 1.0, 1.0, 1.0],
        [1.0, 0.0, 0.0, 1.0],
        [0.0, 1.0, 0.0, 1.0],
        [0.0, 0.0, 1.0, 1.0],
    ];
    let mut output = blank(source.len());
    let limits = ResourceLimits::default();
    RasterToWorkingTransform::new(&srgb_profile(&limits).unwrap(), &limits)
        .unwrap()
        .transform_scanline(&source, &mut output, &limits)
        .unwrap();

    for channel in [output[1].red(), output[1].green(), output[1].blue()] {
        assert_close(channel, 0.003_130_8, 2.0e-5);
    }
    for channel in [output[2].red(), output[2].green(), output[2].blue()] {
        assert_close(channel, 0.214_041_14, 3.0e-5);
    }
    for channel in [output[3].red(), output[3].green(), output[3].blue()] {
        assert_close(channel, 1.0, 3.0e-5);
    }
    for (actual, expected) in [
        (
            [output[4].red(), output[4].green(), output[4].blue()],
            [0.627_404, 0.069_097, 0.016_391],
        ),
        (
            [output[5].red(), output[5].green(), output[5].blue()],
            [0.329_283, 0.919_54, 0.088_013],
        ),
        (
            [output[6].red(), output[6].green(), output[6].blue()],
            [0.043_313, 0.011_362, 0.895_595],
        ),
    ] {
        for (actual, expected) in actual.into_iter().zip(expected) {
            assert_close(actual, expected, 1.5e-4);
        }
    }
}

#[test]
fn rec2020_primaries_gray_and_d65_are_identity() {
    let source = [
        [1.0, 0.0, 0.0, 0.1],
        [0.0, 1.0, 0.0, 0.2],
        [0.0, 0.0, 1.0, 0.3],
        [0.18, 0.18, 0.18, 0.4],
        [1.0, 1.0, 1.0, 0.5],
    ];
    let mut output = blank(source.len());
    let limits = ResourceLimits::default();
    RasterToWorkingTransform::new(&linear_rec2020_profile(&limits).unwrap(), &limits)
        .unwrap()
        .transform_scanline(&source, &mut output, &limits)
        .unwrap();
    for (output, input) in output.iter().zip(source) {
        for (actual, expected) in [output.red(), output.green(), output.blue()]
            .into_iter()
            .zip(input)
            .take(3)
        {
            assert_close(actual, expected, 3.0e-5);
        }
        assert_eq!(output.alpha().to_bits(), input[3].to_bits());
    }
}

#[test]
fn roundtrip_is_close_and_alpha_is_bit_exact_including_subnormal() {
    let source = [
        [0.02, 0.1, 0.9, f32::from_bits(1)],
        [0.25, 0.5, 0.75, -0.0],
        [0.9, 0.4, 0.1, 1.0],
    ];
    let mut working = blank(source.len());
    let limits = ResourceLimits::default();
    let decode_report = RasterToWorkingTransform::new(&srgb_profile(&limits).unwrap(), &limits)
        .unwrap()
        .transform_scanline(&source, &mut working, &limits)
        .unwrap();
    assert_eq!(
        decode_report.working_signal_relation,
        SignalRelation::LinearizedDisplayReferred
    );
    let mut encoded = [[0.0; 4]; 3];
    let report = WorkingToSrgbTransform::new(&limits)
        .unwrap()
        .transform_scanline(
            &working,
            &mut encoded,
            SignalRelation::LinearizedDisplayReferred,
            SdrRangePolicy::Reject,
            &limits,
        )
        .unwrap();
    assert_eq!(report.clipped_samples, 0);
    assert_eq!(
        report.working_signal_relation,
        SignalRelation::LinearizedDisplayReferred
    );
    assert_eq!(report.lcms_version, grainroom::io::color::lcms_version());
    for (actual, expected) in encoded.into_iter().zip(source) {
        for (actual, expected) in actual.into_iter().zip(expected).take(3) {
            assert_close(actual, expected, 2.5e-4);
        }
        assert_eq!(actual[3].to_bits(), expected[3].to_bits());
    }
}

#[test]
fn raster_input_is_bounded_and_errors_are_transactional() {
    let limits = ResourceLimits::default();
    let transform =
        RasterToWorkingTransform::new(&srgb_profile(&limits).unwrap(), &limits).unwrap();
    for invalid in [-f32::MIN_POSITIVE, 1.0001, f32::NAN, f32::INFINITY] {
        let source = [[invalid, 0.5, 0.5, 1.0]];
        let original = RgbaPixel::new(9.0, -2.0, 4.0, 0.25).unwrap();
        let mut destination = [original];
        assert_eq!(
            transform.transform_scanline(&source, &mut destination, &limits),
            Err(ColorError::InvalidRasterSample {
                pixel: 0,
                channel: RasterChannel::Red,
            })
        );
        assert_eq!(destination, [original]);
    }
}

#[test]
fn hdr_working_output_requires_explicit_reject_or_clip_policy() {
    let source = [RgbaPixel::new(4.0, -1.0, 0.5, 0.375).unwrap()];
    let limits = ResourceLimits::default();
    let transform = WorkingToSrgbTransform::new(&limits).unwrap();
    let original = [[0.2, 0.3, 0.4, 0.5]];
    let mut rejected = original;
    assert!(matches!(
        transform.transform_scanline(
            &source,
            &mut rejected,
            SignalRelation::LinearizedDisplayReferred,
            SdrRangePolicy::Reject,
            &limits,
        ),
        Err(ColorError::OutputOutOfRange { .. })
    ));
    assert_eq!(rejected, original);

    let mut clipped = original;
    let report = transform
        .transform_scanline(
            &source,
            &mut clipped,
            SignalRelation::LinearizedDisplayReferred,
            SdrRangePolicy::ClipAndReport,
            &limits,
        )
        .unwrap();
    assert!(report.clipped_samples >= 1);
    assert!(
        clipped[0][..3]
            .iter()
            .all(|sample| (0.0..=1.0).contains(sample))
    );
    assert_eq!(clipped[0][3].to_bits(), source[0].alpha().to_bits());
}

#[test]
fn scene_related_raw_requires_a_display_rendering_stage_before_srgb() {
    let source = [RgbaPixel::new(0.18, 0.18, 0.18, 1.0).unwrap()];
    let limits = ResourceLimits::default();
    let transform = WorkingToSrgbTransform::new(&limits).unwrap();
    let original = [[0.25, 0.5, 0.75, 0.5]];
    let mut destination = original;
    assert_eq!(
        transform.transform_scanline(
            &source,
            &mut destination,
            SignalRelation::SceneRelatedRaw,
            SdrRangePolicy::Reject,
            &limits,
        ),
        Err(ColorError::SceneToDisplayRenderingRequired)
    );
    assert_eq!(destination, original);
}

#[test]
fn transforms_are_deterministic_and_resource_estimates_are_enforced() {
    let source = [[0.13, 0.47, 0.81, 0.7]; 4];
    let limits = ResourceLimits::default();
    let profile = srgb_profile(&limits).unwrap();
    let profile_bytes = profile.icc_provenance().bytes
        + linear_rec2020_profile(&limits)
            .unwrap()
            .icc_provenance()
            .bytes;
    let transform = RasterToWorkingTransform::new(&profile, &limits).unwrap();
    let mut first = blank(4);
    let mut second = blank(4);
    transform
        .transform_scanline(&source, &mut first, &limits)
        .unwrap();
    transform
        .transform_scanline(&source, &mut second, &limits)
        .unwrap();
    assert_eq!(first, second);

    let estimate = estimate_color_working_set(
        4,
        ColorWorkingSetProfile::RasterToWorking,
        profile_bytes,
        &limits,
    )
    .unwrap();
    assert_eq!(estimate.scratch_bytes, 64);
    assert_eq!(estimate.serialized_profile_bytes, profile_bytes);
    assert_eq!(estimate.accounted_bytes, profile_bytes + 64);
    let mut constrained = limits;
    constrained.max_working_bytes = profile_bytes + 63;
    assert!(matches!(
        estimate_color_working_set(
            4,
            ColorWorkingSetProfile::RasterToWorking,
            profile_bytes,
            &constrained
        ),
        Err(ColorError::Limit(
            grainroom::io::LimitError::WorkingBytes { .. }
        ))
    ));
}
