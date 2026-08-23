use serde_json::Value;
use std::process::{Command, Output};

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
fn develop_validates_format_ranges_overrides_and_stops_as_unavailable() {
    let valid = run(&[
        "develop",
        "--input",
        "input.raw",
        "--output",
        "output.JPEG",
        "--quality",
        "90",
        "--set",
        "basics.contrast=-12.5",
    ]);
    assert_eq!(valid.status.code(), Some(69));
    assert!(valid.stdout.is_empty());
    assert!(String::from_utf8_lossy(&valid.stderr).contains("execution is unavailable"));

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
    assert_eq!(valid.status.code(), Some(69));
    assert_eq!(json(&valid)["error"]["code"], "unavailable");
    assert!(valid.stderr.is_empty());

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
