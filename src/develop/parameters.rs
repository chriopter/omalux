use super::DevelopStage;
use std::collections::HashSet;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ParameterUnit {
    Boolean,
    Degrees,
    FilmIso,
    Normalized,
    Percent,
    QuarterTurns,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ParameterKind {
    Scalar,
    Toggle,
    Curve,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ParameterDefinition {
    pub id: String,
    pub label: String,
    pub stage: DevelopStage,
    pub kind: ParameterKind,
    pub unit: ParameterUnit,
    pub minimum: f32,
    pub maximum: f32,
    pub neutral: f32,
    pub step: f32,
}

impl ParameterDefinition {
    fn scalar(
        id: impl Into<String>,
        label: impl Into<String>,
        stage: DevelopStage,
        unit: ParameterUnit,
        range: (f32, f32),
        neutral: f32,
        step: f32,
    ) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            stage,
            kind: ParameterKind::Scalar,
            unit,
            minimum: range.0,
            maximum: range.1,
            neutral,
            step,
        }
    }

    fn toggle(id: impl Into<String>, label: impl Into<String>, stage: DevelopStage) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            stage,
            kind: ParameterKind::Toggle,
            unit: ParameterUnit::Boolean,
            minimum: 0.0,
            maximum: 1.0,
            neutral: 0.0,
            step: 1.0,
        }
    }

    fn curve(id: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            stage: DevelopStage::ToneCurves,
            kind: ParameterKind::Curve,
            unit: ParameterUnit::Normalized,
            minimum: 0.0,
            maximum: 1.0,
            neutral: 0.0,
            step: 0.001,
        }
    }
}

/// Returns the stable registry used by UI generation, CLI validation, and
/// preset tooling. IDs are persisted API and must not be renamed casually.
pub fn parameter_registry() -> Vec<ParameterDefinition> {
    use DevelopStage::*;
    use ParameterUnit::*;

    let mut definitions = vec![
        ParameterDefinition::scalar(
            "geometry.quarter_turns_clockwise",
            "Rotate",
            Geometry,
            QuarterTurns,
            (0.0, 3.0),
            0.0,
            1.0,
        ),
        ParameterDefinition::scalar(
            "geometry.straighten_degrees",
            "Straighten",
            Geometry,
            Degrees,
            (-45.0, 45.0),
            0.0,
            0.1,
        ),
        ParameterDefinition::scalar(
            "geometry.perspective_horizontal",
            "Horizontal perspective",
            Geometry,
            Percent,
            (-100.0, 100.0),
            0.0,
            1.0,
        ),
        ParameterDefinition::scalar(
            "geometry.perspective_vertical",
            "Vertical perspective",
            Geometry,
            Percent,
            (-100.0, 100.0),
            0.0,
            1.0,
        ),
        ParameterDefinition::toggle("geometry.flip_horizontal", "Flip horizontal", Geometry),
        ParameterDefinition::toggle("geometry.flip_vertical", "Flip vertical", Geometry),
    ];
    for (name, range, neutral) in [
        ("x", (0.0, 1.0), 0.0),
        ("y", (0.0, 1.0), 0.0),
        ("width", (f32::EPSILON, 1.0), 1.0),
        ("height", (f32::EPSILON, 1.0), 1.0),
    ] {
        definitions.push(ParameterDefinition::scalar(
            format!("geometry.crop.{name}"),
            format!("Crop {}", title(name)),
            Geometry,
            Normalized,
            range,
            neutral,
            0.001,
        ));
    }

    for name in [
        "brightness",
        "contrast",
        "clarity",
        "highlights",
        "shadows",
        "whites",
        "blacks",
        "saturation",
        "vibrance",
        "temperature",
        "tint",
    ] {
        definitions.push(ParameterDefinition::scalar(
            format!("basics.{name}"),
            title(name),
            Basics,
            Percent,
            (-100.0, 100.0),
            0.0,
            1.0,
        ));
    }

    for channel in ["master", "red", "green", "blue"] {
        definitions.push(ParameterDefinition::curve(
            format!("tone_curves.{channel}"),
            format!("{} curve", title(channel)),
        ));
    }

    for band in [
        "red", "orange", "yellow", "green", "aqua", "blue", "purple", "magenta",
    ] {
        definitions.push(ParameterDefinition::scalar(
            format!("color_mixer.{band}.hue_shift_degrees"),
            format!("{} hue", title(band)),
            ColorMixer,
            Degrees,
            (-180.0, 180.0),
            0.0,
            1.0,
        ));
        for component in ["saturation", "luminance"] {
            definitions.push(ParameterDefinition::scalar(
                format!("color_mixer.{band}.{component}"),
                format!("{} {}", title(band), title(component)),
                ColorMixer,
                Percent,
                (-100.0, 100.0),
                0.0,
                1.0,
            ));
        }
    }

    for range in ["shadows", "midtones", "highlights"] {
        definitions.push(ParameterDefinition::scalar(
            format!("color_grading.{range}.hue_degrees"),
            format!("{} hue", title(range)),
            ColorGrading,
            Degrees,
            (0.0, 360.0),
            0.0,
            1.0,
        ));
        definitions.push(ParameterDefinition::scalar(
            format!("color_grading.{range}.saturation"),
            format!("{} saturation", title(range)),
            ColorGrading,
            Percent,
            (0.0, 100.0),
            0.0,
            1.0,
        ));
        definitions.push(ParameterDefinition::scalar(
            format!("color_grading.{range}.luminance"),
            format!("{} luminance", title(range)),
            ColorGrading,
            Percent,
            (-100.0, 100.0),
            0.0,
            1.0,
        ));
    }
    definitions.push(ParameterDefinition::scalar(
        "color_grading.blending",
        "Blending",
        ColorGrading,
        Percent,
        (0.0, 100.0),
        0.0,
        1.0,
    ));
    definitions.push(ParameterDefinition::scalar(
        "color_grading.balance",
        "Balance",
        ColorGrading,
        Percent,
        (-100.0, 100.0),
        0.0,
        1.0,
    ));

    for (name, range) in [
        ("bloom", (0.0, 100.0)),
        ("halation", (0.0, 100.0)),
        ("fade", (0.0, 100.0)),
        ("vignette", (-100.0, 100.0)),
        ("sharpness", (0.0, 100.0)),
    ] {
        definitions.push(ParameterDefinition::scalar(
            format!("effects.{name}"),
            title(name),
            Effects,
            Percent,
            range,
            0.0,
            1.0,
        ));
    }
    definitions.extend([
        ParameterDefinition::scalar(
            "effects.grain.amount",
            "Grain",
            Effects,
            Percent,
            (0.0, 100.0),
            0.0,
            1.0,
        ),
        ParameterDefinition::scalar(
            "effects.grain.size_iso",
            "Grain size",
            Effects,
            FilmIso,
            (20.0, 6400.0),
            4000.0,
            100.0,
        ),
        ParameterDefinition::scalar(
            "effects.grain.midtone_response",
            "Grain midtones",
            Effects,
            Percent,
            (0.0, 100.0),
            100.0,
            1.0,
        ),
    ]);

    for (name, unit, range, neutral, step) in [
        ("center_x", Normalized, (0.0, 1.0), 0.5, 0.001),
        ("center_y", Normalized, (0.0, 1.0), 0.5, 0.001),
        ("radius_x", Normalized, (f32::EPSILON, 2.0), 0.25, 0.001),
        ("radius_y", Normalized, (f32::EPSILON, 2.0), 0.25, 0.001),
        ("rotation_degrees", Degrees, (-180.0, 180.0), 0.0, 1.0),
        ("feather", Normalized, (0.0, 1.0), 0.5, 0.01),
        ("opacity", Normalized, (0.0, 1.0), 1.0, 0.01),
    ] {
        definitions.push(ParameterDefinition::scalar(
            format!("radial_masks[].{name}"),
            format!("Mask {}", title(name)),
            RadialMasks,
            unit,
            range,
            neutral,
            step,
        ));
    }
    definitions.push(ParameterDefinition::toggle(
        "radial_masks[].enabled",
        "Mask enabled",
        RadialMasks,
    ));
    definitions.push(ParameterDefinition::toggle(
        "radial_masks[].invert",
        "Mask invert",
        RadialMasks,
    ));
    for name in [
        "brightness",
        "contrast",
        "saturation",
        "temperature",
        "tint",
        "sharpness",
    ] {
        definitions.push(ParameterDefinition::scalar(
            format!("radial_masks[].adjustments.{name}"),
            format!("Mask {}", title(name)),
            RadialMasks,
            Percent,
            (-100.0, 100.0),
            0.0,
            1.0,
        ));
    }

    debug_assert_eq!(
        definitions.len(),
        definitions
            .iter()
            .map(|definition| definition.id.as_str())
            .collect::<HashSet<_>>()
            .len(),
        "parameter ids must be unique"
    );
    definitions
}

fn title(value: &str) -> String {
    let value = value.replace('_', " ");
    let mut characters = value.chars();
    match characters.next() {
        Some(first) => first.to_uppercase().collect::<String>() + characters.as_str(),
        None => String::new(),
    }
}
