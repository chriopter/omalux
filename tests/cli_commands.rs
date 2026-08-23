use serde_json::Value;
use std::{
    fs,
    os::unix::fs::symlink,
    process::{Command, Output},
};

fn run(arguments: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_grainroom"))
        .args(arguments)
        .output()
        .unwrap()
}

fn json(output: &Output) -> Value {
    serde_json::from_slice(&output.stdout).unwrap()
}

#[test]
fn catalog_parameter_and_probe_stdout_is_path_free_json() {
    let presets = run(&["presets", "list", "--json"]);
    assert!(presets.status.success());
    assert_eq!(json(&presets)["presets"][0]["id"], "neutral");

    let preset = run(&["presets", "show", "neutral", "--json"]);
    assert!(preset.status.success());
    assert_eq!(json(&preset)["schema_version"], 1);

    let parameters = run(&["parameters", "list", "--json"]);
    assert!(parameters.status.success());
    assert_eq!(
        json(&parameters)["parameters"].as_array().unwrap().len(),
        86
    );

    let probe = run(&["probe", "--json"]);
    assert!(probe.status.success());
    assert!(json(&probe)["raw"]["available"].is_boolean());

    for output in [presets, preset, parameters, probe] {
        assert!(output.stderr.is_empty());
        let text = String::from_utf8(output.stdout).unwrap();
        assert!(!text.contains("/home/"));
        assert!(!text.contains("\\Users\\"));
    }
}

#[test]
fn develop_validates_format_ranges_and_runs_pointwise_settings() {
    let directory = tempfile::tempdir().unwrap();
    let input = directory.path().join("input.jpg");
    let output = directory.path().join("output.JPEG");
    image::save_buffer_with_format(
        &input,
        &[10_u8, 20, 30],
        1,
        1,
        image::ColorType::Rgb8,
        image::ImageFormat::Jpeg,
    )
    .unwrap();
    let valid = Command::new(env!("CARGO_BIN_EXE_grainroom"))
        .arg("develop")
        .arg("--input")
        .arg(&input)
        .arg("--output")
        .arg(&output)
        .args([
            "--quality",
            "90",
            "--set",
            "basics.contrast=-12.5",
            "--set",
            "effects.fade=5",
        ])
        .output()
        .unwrap();
    assert!(valid.status.success());
    assert!(String::from_utf8_lossy(&valid.stdout).contains("develop complete"));
    assert!(valid.stderr.is_empty());
    assert!(output.exists());

    for arguments in [
        vec!["develop", "--input", "in.jpg", "--output", "out.unknown"],
        vec![
            "develop",
            "--input",
            "in.jpg",
            "--output",
            "out.jpg",
            "--quality",
            "0",
        ],
        vec![
            "develop",
            "--input",
            "in.jpg",
            "--output",
            "out.jpg",
            "--set",
            "basics.contrast=1",
            "--set",
            "basics.contrast=2",
        ],
    ] {
        let invalid = run(&arguments);
        assert_eq!(invalid.status.code(), Some(2));
        assert!(invalid.stdout.is_empty());
    }
}

#[test]
fn pointwise_cli_controls_change_jpeg_and_grain_is_rename_deterministic() {
    let directory = tempfile::tempdir().unwrap();
    let input = directory.path().join("gradient.png");
    let mut pixels = Vec::new();
    for y in 0_u8..24 {
        for x in 0_u8..32 {
            pixels.extend_from_slice(&[
                x.saturating_mul(8),
                y.saturating_mul(10),
                x.saturating_mul(4).saturating_add(y.saturating_mul(3)),
            ]);
        }
    }
    image::save_buffer_with_format(
        &input,
        &pixels,
        32,
        24,
        image::ColorType::Rgb8,
        image::ImageFormat::Png,
    )
    .unwrap();

    let develop = |source: &std::path::Path, output: &std::path::Path, setting: Option<&str>| {
        let mut command = Command::new(env!("CARGO_BIN_EXE_grainroom"));
        command
            .arg("develop")
            .arg("--input")
            .arg(source)
            .arg("--output")
            .arg(output)
            .arg("--json");
        if let Some(setting) = setting {
            command.arg("--set").arg(setting);
        }
        command.output().unwrap()
    };

    let neutral = directory.path().join("neutral.jpg");
    let neutral_result = develop(&input, &neutral, None);
    assert!(neutral_result.status.success());
    let neutral_pixels = image::open(&neutral).unwrap().to_rgb8().into_raw();

    for (name, setting) in [
        ("brightness", "basics.brightness=100"),
        ("contrast", "basics.contrast=80"),
        ("fade", "effects.fade=80"),
        ("vignette", "effects.vignette=80"),
        ("grain", "effects.grain.amount=73"),
    ] {
        let output = directory.path().join(format!("{name}.jpg"));
        let result = develop(&input, &output, Some(setting));
        assert!(result.status.success(), "{name}: {:?}", result);
        let report = json(&result);
        assert_eq!(report["schema_version"], 2);
        assert_eq!(report["develop_working_set"]["profile"], "pointwise_v1");
        assert_ne!(
            image::open(output).unwrap().to_rgb8().into_raw(),
            neutral_pixels,
            "{name} did not change the encoded pixels"
        );
    }

    let renamed = directory.path().join("same-content-renamed.png");
    fs::copy(&input, &renamed).unwrap();
    let grain_a = directory.path().join("grain-a.jpg");
    let grain_b = directory.path().join("grain-b.jpg");
    assert!(
        develop(&input, &grain_a, Some("effects.grain.amount=73"))
            .status
            .success()
    );
    assert!(
        develop(&renamed, &grain_b, Some("effects.grain.amount=73"))
            .status
            .success()
    );
    assert_eq!(fs::read(grain_a).unwrap(), fs::read(grain_b).unwrap());
}

#[test]
fn unsupported_cli_setting_fails_before_target_publication() {
    let directory = tempfile::tempdir().unwrap();
    let input = directory.path().join("input.jpg");
    let output = directory.path().join("must-not-exist.jpg");
    image::save_buffer_with_format(
        &input,
        &[70_u8, 80, 90],
        1,
        1,
        image::ColorType::Rgb8,
        image::ImageFormat::Jpeg,
    )
    .unwrap();
    let result = Command::new(env!("CARGO_BIN_EXE_grainroom"))
        .arg("develop")
        .arg("--input")
        .arg(input)
        .arg("--output")
        .arg(&output)
        .arg("--set")
        .arg("basics.clarity=10")
        .arg("--json")
        .output()
        .unwrap();
    assert_eq!(result.status.code(), Some(69));
    assert_eq!(json(&result)["outcome"]["code"], "unproven_pipeline_budget");
    assert!(!output.exists());
    assert!(result.stderr.is_empty());
}

#[test]
fn foundation_flags_are_typed_and_preset_sources_conflict() {
    let valid = run(&[
        "develop",
        "--input",
        "in.png",
        "--output",
        "out.bin",
        "--format",
        "jpg",
        "--unprofiled",
        "reject",
        "--metadata",
        "preserve-safe",
        "--alpha",
        "flatten=#12aBef",
        "--progress",
        "json",
        "--json",
    ]);
    assert_eq!(valid.status.code(), Some(1));
    assert_eq!(json(&valid)["outcome"]["code"], "input_io");
    assert!(!valid.stderr.is_empty());

    let conflict = run(&[
        "develop",
        "--input",
        "in.jpg",
        "--output",
        "out.jpg",
        "--preset",
        "neutral",
        "--preset-file",
        "look.json",
    ]);
    assert_eq!(conflict.status.code(), Some(2));

    let invalid_alpha = run(&[
        "develop",
        "--input",
        "in.jpg",
        "--output",
        "out.jpg",
        "--alpha",
        "flatten=#xyzxyz",
    ]);
    assert_eq!(invalid_alpha.status.code(), Some(2));
}

#[test]
fn legacy_headless_is_a_typed_usage_error() {
    let output = run(&["--headless"]);
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    assert!(String::from_utf8_lossy(&output.stderr).contains("unexpected argument '--headless'"));
}

#[test]
fn unavailable_heic_does_not_open_or_create_any_requested_file() {
    let directory = tempfile::tempdir().unwrap();
    let input = directory.path().join("does-not-exist.raw");
    let preset = directory.path().join("does-not-exist.json");
    let output = directory.path().join("must-not-exist.heic");
    let result = Command::new(env!("CARGO_BIN_EXE_grainroom"))
        .arg("develop")
        .arg("--input")
        .arg(&input)
        .arg("--output")
        .arg(&output)
        .arg("--preset-file")
        .arg(&preset)
        .arg("--json")
        .output()
        .unwrap();
    assert_eq!(result.status.code(), Some(69));
    assert_eq!(json(&result)["error"]["code"], "unavailable");
    assert!(!input.exists());
    assert!(!preset.exists());
    assert!(!output.exists());
    assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 0);
}

#[test]
fn invalid_resource_limits_precede_external_preset_io() {
    use rustix::fs::Mode;

    let directory = tempfile::tempdir().unwrap();
    let preset_fifo = directory.path().join("preset.json");
    rustix::fs::mkfifoat(rustix::fs::CWD, &preset_fifo, Mode::RUSR | Mode::WUSR).unwrap();
    let output = directory.path().join("must-not-exist.jpg");
    let result = Command::new(env!("CARGO_BIN_EXE_grainroom"))
        .arg("develop")
        .arg("--input")
        .arg(directory.path().join("missing.jpg"))
        .arg("--output")
        .arg(&output)
        .arg("--preset-file")
        .arg(&preset_fifo)
        .arg("--max-source-bytes")
        .arg("0")
        .output()
        .unwrap();
    assert_eq!(result.status.code(), Some(2));
    assert!(preset_fifo.exists());
    assert!(!output.exists());
}

#[cfg(target_os = "linux")]
#[test]
fn gui_command_rejects_a_sibling_symlink_and_accepts_a_regular_held_sibling() {
    let directory = tempfile::tempdir().unwrap();
    let core = directory.path().join("grainroom");
    let sibling = directory.path().join("grainroom-gui");
    fs::copy(env!("CARGO_BIN_EXE_grainroom"), &core).unwrap();

    symlink("/bin/true", &sibling).unwrap();
    assert_eq!(
        Command::new(&core).arg("gui").status().unwrap().code(),
        Some(69)
    );

    fs::remove_file(&sibling).unwrap();
    fs::copy("/bin/true", &sibling).unwrap();
    assert!(Command::new(&core).arg("gui").status().unwrap().success());
}
