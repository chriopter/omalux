use omalux::develop::{
    DevelopSettings, ParameterKind, ParameterOverride, ParameterOverrideError,
    ParameterOverrideValue, apply_parameter_overrides, parameter_registry,
    parse_parameter_override,
};

#[test]
fn parser_is_strict_typed_and_registry_bounded() {
    assert_eq!(
        parse_parameter_override("basics.contrast=-12.5")
            .unwrap()
            .value(),
        ParameterOverrideValue::Scalar(-12.5)
    );
    assert_eq!(
        parse_parameter_override("geometry.flip_horizontal=true")
            .unwrap()
            .value(),
        ParameterOverrideValue::Toggle(true)
    );
    for invalid in [
        "basics.contrast",
        "=1",
        "basics.contrast=",
        "basics.contrast=1=2",
    ] {
        assert!(matches!(
            parse_parameter_override(invalid),
            Err(ParameterOverrideError::InvalidExpression)
        ));
    }
    assert!(matches!(
        parse_parameter_override("missing=1"),
        Err(ParameterOverrideError::UnknownParameter(_))
    ));
    for value in ["NaN", "inf", "101"] {
        assert!(matches!(
            parse_parameter_override(&format!("basics.contrast={value}")),
            Err(ParameterOverrideError::ScalarOutOfRange { .. })
                | Err(ParameterOverrideError::InvalidScalar(_))
        ));
    }
    assert!(matches!(
        parse_parameter_override("geometry.flip_horizontal=1"),
        Err(ParameterOverrideError::InvalidToggle(_))
    ));
}

#[test]
fn every_registry_scalar_and_toggle_is_mapped_or_typed_structured() {
    for definition in parameter_registry() {
        let probe = if definition.neutral + definition.step <= definition.maximum {
            definition.neutral + definition.step
        } else {
            definition.neutral - definition.step
        };
        let expression = match definition.kind {
            ParameterKind::Scalar => format!("{}={probe}", definition.id),
            ParameterKind::Toggle => format!("{}=true", definition.id),
            _ => format!("{}=ignored", definition.id),
        };
        let parsed = parse_parameter_override(&expression);
        let structured = definition.id.starts_with("tone_curves.")
            || definition.id == "radial_masks"
            || definition.id.starts_with("radial_masks[]")
            || definition.id == "geometry.crop.enabled";
        if structured
            || !matches!(
                definition.kind,
                ParameterKind::Scalar | ParameterKind::Toggle
            )
        {
            assert!(
                matches!(parsed, Err(ParameterOverrideError::StructuredParameter(ref id)) if id == &definition.id),
                "{} was not rejected as structured",
                definition.id
            );
        } else {
            let parsed = parsed.unwrap_or_else(|error| panic!("{}: {error}", definition.id));
            let mut base = DevelopSettings::default();
            if definition.id == "geometry.crop.x" {
                base.geometry.crop = Some(omalux::develop::CropRect {
                    x: 0.0,
                    y: 0.0,
                    width: 0.5,
                    height: 1.0,
                });
            } else if definition.id == "geometry.crop.y" {
                base.geometry.crop = Some(omalux::develop::CropRect {
                    x: 0.0,
                    y: 0.0,
                    width: 1.0,
                    height: 0.5,
                });
            }
            let settings = apply_parameter_overrides(&base, &[parsed])
                .unwrap_or_else(|error| panic!("{}: {error}", definition.id));
            let json = serde_json::to_value(settings).unwrap();
            let actual = definition
                .id
                .split('.')
                .fold(&json, |value, component| &value[component]);
            match definition.kind {
                ParameterKind::Scalar => assert!(
                    actual
                        .as_f64()
                        .is_some_and(|actual| (actual - f64::from(probe)).abs() < 1.0e-5),
                    "{} mapped to the wrong field: {actual}",
                    definition.id
                ),
                ParameterKind::Toggle => assert_eq!(
                    actual.as_bool(),
                    Some(true),
                    "{} mapped to the wrong field",
                    definition.id
                ),
                _ => unreachable!(),
            }
        }
    }
}

#[test]
fn overrides_apply_transactionally_once_then_canonicalize() {
    let overrides = [
        parse_parameter_override("geometry.crop.x=0.25").unwrap(),
        parse_parameter_override("geometry.crop.width=0.75").unwrap(),
        parse_parameter_override("geometry.flip_horizontal=true").unwrap(),
        parse_parameter_override("basics.brightness=12.5").unwrap(),
        parse_parameter_override("color_mixer.blue.hue_shift_degrees=180").unwrap(),
        parse_parameter_override("color_grading.highlights.hue_degrees=360").unwrap(),
        parse_parameter_override("effects.grain.size_iso=800").unwrap(),
    ];
    let settings = apply_parameter_overrides(&DevelopSettings::default(), &overrides).unwrap();
    let crop = settings.geometry.crop.unwrap();
    assert_eq!((crop.x, crop.width), (0.25, 0.75));
    assert!(settings.geometry.flip_horizontal);
    assert_eq!(settings.basics.brightness, 12.5);
    assert_eq!(settings.color_mixer.blue.hue_shift_degrees, -180.0);
    assert_eq!(settings.color_grading.highlights.hue_degrees, 0.0);
    assert_eq!(settings.effects.grain.size_iso, 800.0);
}

#[test]
fn wrong_kinds_duplicates_and_invalid_composites_fail_loudly() {
    assert!(matches!(
        apply_parameter_overrides(
            &DevelopSettings::default(),
            &[ParameterOverride::toggle("basics.contrast", true)]
        ),
        Err(ParameterOverrideError::WrongValueKind(_))
    ));
    assert!(matches!(
        apply_parameter_overrides(
            &DevelopSettings::default(),
            &[
                ParameterOverride::scalar("basics.contrast", 1.0),
                ParameterOverride::scalar("basics.contrast", 2.0),
            ]
        ),
        Err(ParameterOverrideError::DuplicateParameter(_))
    ));
    assert!(matches!(
        apply_parameter_overrides(
            &DevelopSettings::default(),
            &[ParameterOverride::scalar("geometry.crop.x", 0.5)]
        ),
        Err(ParameterOverrideError::InvalidSettings(_))
    ));
    assert!(matches!(
        apply_parameter_overrides(
            &DevelopSettings::default(),
            &[ParameterOverride::scalar(
                "geometry.quarter_turns_clockwise",
                1.5
            )]
        ),
        Err(ParameterOverrideError::IntegerRequired(_))
    ));
    assert_eq!(DevelopSettings::default(), DevelopSettings::default());
}
