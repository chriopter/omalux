use grainroom::develop::{
    CpuImage, CropRect, CurvePoint, DevelopPipeline, DevelopRenderContext, DevelopSettings,
    DevelopStage, DevelopWorkingSetProfile, LocalAdjustments, PipelineError, RadialMask, RgbaPixel,
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
fn spatial_v1_gates_each_spatial_family_at_its_conservative_peak() {
    type SpatialCase = (fn(&mut DevelopSettings), u64);
    let cases: [SpatialCase; 4] = [
        (|settings| settings.basics.clarity = 50.0, 1_744),
        (|settings| settings.effects.bloom = 50.0, 1_480),
        (|settings| settings.effects.halation = 50.0, 1_480),
        (|settings| settings.effects.sharpness = 50.0, 1_480),
    ];
    for (configure, peak) in cases {
        let mut settings = DevelopSettings::default();
        configure(&mut settings);
        let exact = ResourceLimits::default().with_max_working_bytes(peak);
        let estimate = estimate_develop_working_set(4, 3, &settings, &exact).unwrap();
        assert_eq!(estimate.profile, DevelopWorkingSetProfile::SpatialV1);
        assert_eq!(estimate.peak_bytes, peak);

        let mut candidate = image(4, 3);
        DevelopPipeline
            .process_bounded(&mut candidate, &settings, &exact)
            .unwrap();

        let below = ResourceLimits::default().with_max_working_bytes(peak - 1);
        let mut rejected = image(4, 3);
        let original = rejected.clone();
        assert_eq!(
            DevelopPipeline.process_bounded(&mut rejected, &settings, &below),
            Err(PipelineError::ResourceLimit(LimitError::WorkingBytes {
                requested: peak,
                maximum: peak - 1,
            }))
        );
        assert_eq!(rejected, original);
    }
}

#[test]
fn combined_spatial_stages_use_the_max_sequential_stage_peak() {
    let mut settings = DevelopSettings::default();
    settings.basics.clarity = 20.0;
    settings.effects.bloom = 30.0;
    settings.effects.halation = 40.0;
    settings.effects.sharpness = 50.0;
    let estimate = estimate_develop_working_set(
        4,
        3,
        &settings,
        &ResourceLimits::default().with_max_working_bytes(1_744),
    )
    .unwrap();
    assert_eq!(estimate.profile, DevelopWorkingSetProfile::SpatialV1);
    assert_eq!(estimate.stage_scratch_bytes, 1_360);
    assert_eq!(estimate.peak_bytes, 1_744);
}

fn active_mask(invert: bool, sharpness: f32) -> RadialMask {
    RadialMask {
        id: "budget-mask".into(),
        enabled: true,
        center_x: 0.5,
        center_y: 0.5,
        radius_x: 0.25,
        radius_y: 0.25,
        rotation_degrees: 0.0,
        feather: 0.1,
        opacity: 1.0,
        invert,
        adjustments: LocalAdjustments {
            brightness: 1.0,
            sharpness,
            ..LocalAdjustments::default()
        },
    }
}

#[test]
fn geometry_v1_crop_rotate_and_perspective_have_exact_simultaneous_peaks() {
    type GeometryCase = (fn(&mut DevelopSettings), u64, (u32, u32));
    let cases: [GeometryCase; 3] = [
        (
            |settings| settings.geometry.quarter_turns_clockwise = 1,
            576,
            (3, 4),
        ),
        (
            |settings| settings.geometry.perspective_horizontal = 10.0,
            576,
            (4, 3),
        ),
        (
            |settings| {
                settings.geometry.crop = Some(CropRect {
                    x: 0.0,
                    y: 0.0,
                    width: 0.5,
                    height: 1.0,
                });
            },
            480,
            (2, 3),
        ),
    ];
    for (configure, exact_peak, dimensions) in cases {
        let mut settings = DevelopSettings::default();
        configure(&mut settings);
        let exact = ResourceLimits::default().with_max_working_bytes(exact_peak);
        let estimate = estimate_develop_working_set(4, 3, &settings, &exact).unwrap();
        assert!(estimate.profile.geometry_v1);
        assert_eq!(estimate.peak_bytes, exact_peak);
        let mut rendered = image(4, 3);
        DevelopPipeline
            .process_bounded(&mut rendered, &settings, &exact)
            .unwrap();
        assert_eq!((rendered.width(), rendered.height()), dimensions);

        let below = ResourceLimits::default().with_max_working_bytes(exact_peak - 1);
        let mut rejected = image(4, 3);
        let original = rejected.clone();
        assert_eq!(
            DevelopPipeline.process_bounded(&mut rejected, &settings, &below),
            Err(PipelineError::ResourceLimit(LimitError::WorkingBytes {
                requested: exact_peak,
                maximum: exact_peak - 1,
            }))
        );
        assert_eq!(rejected, original);
    }
}

#[test]
fn geometry_v1_accounts_two_live_stage_images_for_rotate_then_projective() {
    let mut settings = DevelopSettings::default();
    settings.geometry.quarter_turns_clockwise = 1;
    settings.geometry.straighten_degrees = 2.0;
    settings.geometry.crop = Some(CropRect {
        x: 0.25,
        y: 0.25,
        width: 0.5,
        height: 0.5,
    });
    let exact = ResourceLimits::default().with_max_working_bytes(768);
    let estimate = estimate_develop_working_set(4, 3, &settings, &exact).unwrap();
    assert_eq!(estimate.peak_bytes, 768);
    assert_eq!(estimate.stage_scratch_bytes, 384);
}

#[test]
fn radial_masks_v1_accounts_largest_roi_and_full_invert_without_mask_planes() {
    let mut roi = DevelopSettings::default();
    roi.radial_masks.masks.push(active_mask(false, 20.0));
    let roi_estimate =
        estimate_develop_working_set(8, 6, &roi, &ResourceLimits::default()).unwrap();
    assert!(roi_estimate.profile.radial_masks_v1);
    assert!(roi_estimate.stage_scratch_bytes > 0);
    assert!(roi_estimate.stage_scratch_bytes < 8 * 6 * 16);

    let mut full = DevelopSettings::default();
    full.radial_masks.masks.push(active_mask(true, 20.0));
    let exact_peak = 8 * 6 * 48;
    let exact = ResourceLimits::default().with_max_working_bytes(exact_peak);
    let estimate = estimate_develop_working_set(8, 6, &full, &exact).unwrap();
    assert_eq!(estimate.stage_scratch_bytes, 8 * 6 * 16);
    assert_eq!(estimate.peak_bytes, exact_peak);
    let mut rendered = image(8, 6);
    DevelopPipeline
        .process_bounded(&mut rendered, &full, &exact)
        .unwrap();

    let below = ResourceLimits::default().with_max_working_bytes(exact_peak - 1);
    let mut rejected = image(8, 6);
    let original = rejected.clone();
    assert!(
        DevelopPipeline
            .process_bounded(&mut rejected, &full, &below)
            .is_err()
    );
    assert_eq!(rejected, original);
}

#[test]
fn geometry_masks_compose_explicitly_with_color_and_spatial_profiles() {
    let mut settings = DevelopSettings::default();
    settings.geometry.quarter_turns_clockwise = 1;
    settings.tone_curves.master.points[1].y = 0.8;
    settings.basics.clarity = 15.0;
    settings.radial_masks.masks.push(active_mask(false, 0.0));
    let estimate =
        estimate_develop_working_set(8, 6, &settings, &ResourceLimits::default()).unwrap();
    assert_eq!(
        estimate.profile,
        DevelopWorkingSetProfile::new(true, true, true, true)
    );
    let mut rendered = image(8, 6);
    DevelopPipeline
        .process_bounded(&mut rendered, &settings, &ResourceLimits::default())
        .unwrap();
}

#[test]
fn negative_local_sharpness_remains_unsupported_before_mutation() {
    let mut settings = DevelopSettings::default();
    settings.radial_masks.masks.push(active_mask(false, -1.0));
    assert_unproven(
        &settings,
        DevelopStage::RadialMasks,
        &ResourceLimits::default(),
    );
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
fn color_spatial_v1_uses_the_larger_sequential_scratch_peak() {
    let mut color_dominant = DevelopSettings::default();
    color_dominant.tone_curves.master.points = max_curve(0.10);
    color_dominant.tone_curves.red.points = max_curve(0.20);
    color_dominant.tone_curves.green.points = max_curve(0.30);
    color_dominant.tone_curves.blue.points = max_curve(0.40);
    color_dominant.basics.clarity = 20.0;
    let color_peak = 384 + 4 * 31 * 7 * 8;
    let color_estimate = estimate_develop_working_set(
        4,
        3,
        &color_dominant,
        &ResourceLimits::default().with_max_working_bytes(color_peak),
    )
    .unwrap();
    assert_eq!(
        color_estimate.profile,
        DevelopWorkingSetProfile::ColorSpatialV1
    );
    assert_eq!(color_estimate.stage_scratch_bytes, 4 * 31 * 7 * 8);

    let mut spatial_dominant = DevelopSettings::default();
    spatial_dominant.tone_curves.master.points[1].y = 0.8;
    spatial_dominant.basics.clarity = 20.0;
    let spatial_peak = 1_744;
    let exact = ResourceLimits::default().with_max_working_bytes(spatial_peak);
    let spatial_estimate = estimate_develop_working_set(4, 3, &spatial_dominant, &exact).unwrap();
    assert_eq!(
        spatial_estimate.profile,
        DevelopWorkingSetProfile::ColorSpatialV1
    );
    assert_eq!(spatial_estimate.stage_scratch_bytes, 1_360);
    assert_eq!(spatial_estimate.peak_bytes, spatial_peak);

    let mut rendered = image(4, 3);
    DevelopPipeline
        .process_bounded(&mut rendered, &spatial_dominant, &exact)
        .unwrap();
    let below = ResourceLimits::default().with_max_working_bytes(spatial_peak - 1);
    let mut rejected = image(4, 3);
    let original = rejected.clone();
    assert_eq!(
        DevelopPipeline.process_bounded(&mut rejected, &spatial_dominant, &below),
        Err(PipelineError::ResourceLimit(LimitError::WorkingBytes {
            requested: spatial_peak,
            maximum: spatial_peak - 1,
        }))
    );
    assert_eq!(rejected, original);
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
    assert!(matches!(
        DevelopPipeline.process_bounded(&mut candidate, settings, limits),
        Err(PipelineError::ResourceProfileUnavailable(value))
            | Err(PipelineError::StageNotImplemented(value)) if value == stage
    ));
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
