use grainroom::develop::{
    CpuImage, CurvePoint, DevelopPipeline, DevelopRenderContext, DevelopSettings, DevelopStage,
    DevelopWorkingSetProfile, LocalAdjustments, PipelineError, RadialMask, RgbaPixel,
    estimate_develop_working_set,
};
use grainroom::io::{LimitError, ResourceLimits};

fn image(width: u32, height: u32) -> CpuImage {
    let pixel = RgbaPixel::new(0.25, 0.5, 1.25, 0.75).unwrap();
    CpuImage::new(width, height, vec![pixel; (width * height) as usize]).unwrap()
}

fn pointwise_settings() -> DevelopSettings {
    let mut settings = DevelopSettings::default();
    settings.basics.brightness = 12.0;
    settings.basics.contrast = -8.0;
    settings.basics.saturation = 7.0;
    settings.effects.fade = 5.0;
    settings.effects.vignette = -13.0;
    settings.effects.grain.amount = 24.0;
    settings
}

#[test]
fn pointwise_v1_has_an_exact_named_peak_and_peak_minus_one_fails() {
    let settings = pointwise_settings();
    let exact = ResourceLimits::default().with_max_working_bytes(4 * 3 * 32);
    let estimate = estimate_develop_working_set(4, 3, &settings, &exact).unwrap();
    assert_eq!(estimate.profile, DevelopWorkingSetProfile::PointwiseV1);
    assert_eq!(estimate.pixels, 12);
    assert_eq!(estimate.source_image_bytes, 192);
    assert_eq!(estimate.transactional_image_bytes, 192);
    assert_eq!(estimate.stage_scratch_bytes, 0);
    assert_eq!(estimate.peak_bytes, 384);

    let below = ResourceLimits::default().with_max_working_bytes(383);
    assert_eq!(
        estimate_develop_working_set(4, 3, &settings, &below),
        Err(PipelineError::ResourceLimit(LimitError::WorkingBytes {
            requested: 384,
            maximum: 383,
        }))
    );
}

#[test]
fn bounded_render_is_transactional_and_matches_unbounded_render() {
    let settings = pointwise_settings();
    let context = DevelopRenderContext::from_source_digest([0x42; 32]);
    let mut expected = image(4, 3);
    DevelopPipeline
        .process_with_context(&mut expected, &settings, Some(&context))
        .unwrap();

    let exact = ResourceLimits::default().with_max_working_bytes(384);
    let mut actual = image(4, 3);
    let estimate = DevelopPipeline
        .process_bounded_with_context(&mut actual, &settings, Some(&context), &exact)
        .unwrap();
    assert_eq!(estimate.peak_bytes, 384);
    assert_eq!(actual, expected);

    let below = ResourceLimits::default().with_max_working_bytes(383);
    let mut unchanged = image(4, 3);
    let original = unchanged.clone();
    assert!(matches!(
        DevelopPipeline.process_bounded_with_context(
            &mut unchanged,
            &settings,
            Some(&context),
            &below
        ),
        Err(PipelineError::ResourceLimit(
            LimitError::WorkingBytes { .. }
        ))
    ));
    assert_eq!(unchanged, original);
}

#[test]
fn every_unproven_spatial_family_fails_closed_before_rendering() {
    let limits = ResourceLimits::default();

    let mut clarity = DevelopSettings::default();
    clarity.basics.clarity = 1.0;
    assert_unproven(&clarity, DevelopStage::Basics, &limits);

    let mut geometry = DevelopSettings::default();
    geometry.geometry.quarter_turns_clockwise = 1;
    assert_unproven(&geometry, DevelopStage::Geometry, &limits);

    let mut radial = DevelopSettings::default();
    radial.radial_masks.masks.push(RadialMask {
        id: "budget-gate".into(),
        enabled: true,
        center_x: 0.5,
        center_y: 0.5,
        radius_x: 0.25,
        radius_y: 0.25,
        rotation_degrees: 0.0,
        feather: 0.1,
        opacity: 1.0,
        invert: false,
        adjustments: LocalAdjustments {
            brightness: 1.0,
            ..LocalAdjustments::default()
        },
    });
    assert_unproven(&radial, DevelopStage::RadialMasks, &limits);

    for configure in [
        |settings: &mut DevelopSettings| settings.effects.bloom = 1.0,
        |settings: &mut DevelopSettings| settings.effects.halation = 1.0,
        |settings: &mut DevelopSettings| settings.effects.sharpness = 1.0,
    ] {
        let mut effects = DevelopSettings::default();
        configure(&mut effects);
        assert_unproven(&effects, DevelopStage::Effects, &limits);
    }
}

fn max_curve(offset: f32) -> Vec<CurvePoint> {
    (0..32)
        .map(|index| {
            let x = index as f32 / 31.0;
            CurvePoint {
                x,
                y: (x * (1.0 - offset) + offset * x * x).clamp(0.0, 1.0),
            }
        })
        .collect()
}

#[test]
fn color_v1_max_curves_have_an_exact_peak_and_apply_all_color_stages() {
    let mut settings = pointwise_settings();
    settings.tone_curves.master.points = max_curve(0.10);
    settings.tone_curves.red.points = max_curve(0.20);
    settings.tone_curves.green.points = max_curve(0.30);
    settings.tone_curves.blue.points = max_curve(0.40);
    settings.color_mixer.blue.saturation = 17.0;
    settings.color_grading.midtones.hue_degrees = 210.0;
    settings.color_grading.midtones.saturation = 12.0;

    // Four 31-segment curves, each segment is seven f64 coefficients.
    let curve_payload = 4 * 31 * 7 * 8;
    let exact_peak = 2 * 2 * 32 + curve_payload;
    let exact = ResourceLimits::default().with_max_working_bytes(exact_peak);
    let estimate = estimate_develop_working_set(2, 2, &settings, &exact).unwrap();
    assert_eq!(estimate.profile, DevelopWorkingSetProfile::ColorV1);
    assert_eq!(estimate.stage_scratch_bytes, curve_payload);
    assert_eq!(estimate.peak_bytes, exact_peak);

    let original = image(2, 2);
    let mut rendered = original.clone();
    let context = DevelopRenderContext::from_source_digest([0x25; 32]);
    DevelopPipeline
        .process_bounded_with_context(&mut rendered, &settings, Some(&context), &exact)
        .unwrap();
    assert_ne!(rendered, original);

    let below = ResourceLimits::default().with_max_working_bytes(exact_peak - 1);
    let mut unchanged = original.clone();
    assert_eq!(
        DevelopPipeline.process_bounded_with_context(
            &mut unchanged,
            &settings,
            Some(&context),
            &below
        ),
        Err(PipelineError::ResourceLimit(LimitError::WorkingBytes {
            requested: exact_peak,
            maximum: exact_peak - 1,
        }))
    );
    assert_eq!(unchanged, original);
}

#[test]
fn mixer_and_grading_add_no_heap_scratch() {
    let mut settings = DevelopSettings::default();
    settings.color_mixer.red.hue_shift_degrees = 15.0;
    settings.color_grading.highlights.saturation = 20.0;
    let estimate = estimate_develop_working_set(
        2,
        2,
        &settings,
        &ResourceLimits::default().with_max_working_bytes(128),
    )
    .unwrap();
    assert_eq!(estimate.profile, DevelopWorkingSetProfile::ColorV1);
    assert_eq!(estimate.stage_scratch_bytes, 0);
    assert_eq!(estimate.peak_bytes, 128);
}

#[test]
fn malformed_curve_is_rejected_before_clone_or_mutation() {
    let mut settings = DevelopSettings::default();
    settings.tone_curves.master.points = vec![
        CurvePoint { x: 0.0, y: 0.0 },
        CurvePoint { x: 0.5, y: 0.8 },
        CurvePoint { x: 0.5, y: 0.9 },
        CurvePoint { x: 1.0, y: 1.0 },
    ];
    let mut candidate = image(2, 2);
    let original = candidate.clone();
    assert!(matches!(
        DevelopPipeline.process_bounded(&mut candidate, &settings, &ResourceLimits::default()),
        Err(PipelineError::InvalidSettings(_))
    ));
    assert_eq!(candidate, original);
}

fn assert_unproven(settings: &DevelopSettings, stage: DevelopStage, limits: &ResourceLimits) {
    assert_eq!(
        estimate_develop_working_set(2, 2, settings, limits),
        Err(PipelineError::ResourceProfileUnavailable(stage))
    );
    let mut candidate = image(2, 2);
    let original = candidate.clone();
    assert_eq!(
        DevelopPipeline.process_bounded(&mut candidate, settings, limits),
        Err(PipelineError::ResourceProfileUnavailable(stage))
    );
    assert_eq!(candidate, original);
}

#[test]
fn dimensions_and_checked_arithmetic_are_rejected_before_allocation() {
    assert_eq!(
        estimate_develop_working_set(
            0,
            1,
            &DevelopSettings::default(),
            &ResourceLimits::default()
        ),
        Err(PipelineError::ResourceLimit(LimitError::EmptyDimensions))
    );
    assert_eq!(
        estimate_develop_working_set(
            u32::MAX,
            u32::MAX,
            &DevelopSettings::default(),
            &ResourceLimits::default()
        ),
        Err(PipelineError::ResourceLimit(LimitError::PixelCount {
            requested: u64::from(u32::MAX) * u64::from(u32::MAX),
            maximum: ResourceLimits::default().max_pixels,
        }))
    );
}

#[test]
fn sixty_four_dormant_masks_keep_the_pointwise_exact_profile() {
    assert_eq!(std::mem::size_of::<RgbaPixel>(), 16);
    let mut settings = DevelopSettings::default();
    for index in 0..64 {
        settings.radial_masks.masks.push(RadialMask {
            id: format!("dormant-{index}"),
            enabled: false,
            center_x: 0.5,
            center_y: 0.5,
            radius_x: 0.25,
            radius_y: 0.25,
            rotation_degrees: 0.0,
            feather: 0.1,
            opacity: 1.0,
            invert: false,
            adjustments: LocalAdjustments {
                brightness: 1.0,
                ..LocalAdjustments::default()
            },
        });
    }
    let limits = ResourceLimits::default().with_max_working_bytes(128);
    let estimate = estimate_develop_working_set(2, 2, &settings, &limits).unwrap();
    assert_eq!(estimate.profile, DevelopWorkingSetProfile::PointwiseV1);
    assert_eq!(estimate.peak_bytes, 128);

    let mut candidate = image(2, 2);
    DevelopPipeline
        .process_bounded(&mut candidate, &settings, &limits)
        .unwrap();
    assert_eq!(candidate, image(2, 2));
}
