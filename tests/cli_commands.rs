use grainroom::develop::{CurvePoint, DevelopSettings, PresetDocument};
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
    assert!(json(&probe)["heic"]["available"].is_boolean());

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
        assert_eq!(report["schema_version"], 3);
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
fn external_structured_curve_and_scalar_color_overrides_run_as_color_v1() {
    let directory = tempfile::tempdir().unwrap();
    let input = directory.path().join("input.png");
    let output = directory.path().join("output.jpg");
    let neutral_output = directory.path().join("neutral.jpg");
    let preset_path = directory.path().join("color.json");
    image::save_buffer_with_format(
        &input,
        &[40_u8, 90, 180, 160, 80, 20],
        2,
        1,
        image::ColorType::Rgb8,
        image::ImageFormat::Png,
    )
    .unwrap();
    let mut settings = DevelopSettings::default();
    settings.tone_curves.master.points = vec![
        CurvePoint { x: 0.0, y: 0.0 },
        CurvePoint { x: 0.5, y: 0.7 },
        CurvePoint { x: 1.0, y: 1.0 },
    ];
    let preset = PresetDocument::new("color-v1-cli", "Color V1 CLI", settings);
    fs::write(&preset_path, preset.to_canonical_json().unwrap()).unwrap();

    let neutral = Command::new(env!("CARGO_BIN_EXE_grainroom"))
        .arg("develop")
        .arg("--input")
        .arg(&input)
        .arg("--output")
        .arg(&neutral_output)
        .output()
        .unwrap();
    assert!(neutral.status.success());

    let result = Command::new(env!("CARGO_BIN_EXE_grainroom"))
        .arg("develop")
        .arg("--input")
        .arg(&input)
        .arg("--output")
        .arg(&output)
        .arg("--preset-file")
        .arg(&preset_path)
        .arg("--set")
        .arg("color_mixer.blue.saturation=25")
        .arg("--set")
        .arg("color_grading.midtones.hue_degrees=215")
        .arg("--set")
        .arg("color_grading.midtones.saturation=20")
        .arg("--json")
        .output()
        .unwrap();
    assert!(result.status.success(), "{result:?}");
    let report = json(&result);
    assert_eq!(report["schema_version"], 3);
    assert_eq!(report["output_format"], "jpeg");
    assert_eq!(report["encoding"]["format"], "jpeg");
    assert_eq!(report["develop_working_set"]["profile"], "color_v1");
    assert!(output.exists());
    assert_ne!(
        image::open(&output).unwrap().to_rgb8(),
        image::open(&neutral_output).unwrap().to_rgb8()
    );
    assert!(result.stderr.is_empty());
}

#[test]
fn unsupported_spatial_cli_setting_fails_before_target_publication() {
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
        .arg("geometry.straighten_degrees=1")
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

#[cfg(not(feature = "heic"))]
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

#[cfg(not(feature = "heic"))]
#[test]
fn unavailable_heic_still_reports_option_usage_before_any_file_io() {
    use rustix::fs::Mode;

    let directory = tempfile::tempdir().unwrap();
    let preset = directory.path().join("preset.json");
    rustix::fs::mkfifoat(rustix::fs::CWD, &preset, Mode::RUSR | Mode::WUSR).unwrap();
    let output = directory.path().join("must-not-exist.heic");
    let result = Command::new(env!("CARGO_BIN_EXE_grainroom"))
        .args(["develop", "--input"])
        .arg(directory.path().join("missing.png"))
        .arg("--output")
        .arg(&output)
        .arg("--preset-file")
        .arg(&preset)
        .args(["--max-source-bytes", "0", "--json"])
        .output()
        .unwrap();
    assert_eq!(result.status.code(), Some(2));
    assert!(result.stdout.is_empty());
    assert!(preset.exists());
    assert!(!output.exists());
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

#[cfg(feature = "heic")]
#[test]
fn production_heic_cli_encodes_ten_bit_and_reports_path_free_provenance() {
    let directory = tempfile::tempdir().unwrap();
    let input = directory.path().join("gradient.png");
    let output = directory.path().join("developed.heic");
    let preset_path = directory.path().join("color.json");
    let mut pixels = Vec::new();
    for y in 0_u8..3 {
        for x in 0_u8..5 {
            pixels.extend_from_slice(&[x * 40, y * 70, x * 20 + y * 15]);
        }
    }
    image::save_buffer_with_format(
        &input,
        &pixels,
        5,
        3,
        image::ColorType::Rgb8,
        image::ImageFormat::Png,
    )
    .unwrap();
    let mut settings = DevelopSettings::default();
    settings.tone_curves.master.points = vec![
        CurvePoint { x: 0.0, y: 0.0 },
        CurvePoint { x: 0.5, y: 0.65 },
        CurvePoint { x: 1.0, y: 1.0 },
    ];
    let preset = PresetDocument::new("color-v1-heic", "Color V1 HEIC", settings);
    fs::write(&preset_path, preset.to_canonical_json().unwrap()).unwrap();
    let result = Command::new(env!("CARGO_BIN_EXE_grainroom"))
        .args(["develop", "--input"])
        .arg(&input)
        .arg("--output")
        .arg(&output)
        .args(["--format", "heic", "--quality", "90"])
        .arg("--preset-file")
        .arg(&preset_path)
        .args([
            "--set",
            "color_mixer.blue.saturation=25",
            "--set",
            "color_grading.midtones.hue_degrees=215",
            "--set",
            "color_grading.midtones.saturation=20",
            "--json",
        ])
        .output()
        .unwrap();
    assert!(result.status.success(), "{result:?}");
    assert!(result.stderr.is_empty());
    let report = json(&result);
    assert_eq!(report["schema_version"], 3);
    assert_eq!(report["output_format"], "heic");
    assert_eq!(report["encoding"]["format"], "heic");
    assert_eq!(report["encoding"]["quality"], 90);
    assert_eq!(report["encoding"]["bit_depth"], 10);
    assert_eq!(report["encoding"]["nclx"]["color_primaries"], 1);
    assert_eq!(report["encoding"]["nclx"]["transfer_characteristics"], 13);
    assert_eq!(report["encoding"]["nclx"]["matrix_coefficients"], 1);
    assert_eq!(report["encoding"]["nclx"]["full_range"], true);
    assert_eq!(report["develop_working_set"]["profile"], "color_v1");
    assert!(
        report["encoding"]["encoder"]
            .as_str()
            .unwrap()
            .contains("x265")
    );
    assert!(
        !report["encoding"]["libheif_version"]
            .as_str()
            .unwrap()
            .is_empty()
    );
    let rendered = serde_json::to_string(&report).unwrap();
    assert!(!rendered.contains(directory.path().to_str().unwrap()));
    let bytes = fs::read(&output).unwrap();
    assert!(bytes.windows(4).any(|window| window == b"ftyp"));
    unsafe { assert_heic_dimensions_and_depth(&bytes, 5, 3, 10) };
}

#[cfg(feature = "heic")]
unsafe fn assert_heic_dimensions_and_depth(bytes: &[u8], width: u32, height: u32, depth: i32) {
    use libheif_sys as heif;
    use std::ptr;

    let context = unsafe { heif::heif_context_alloc() };
    assert!(!context.is_null());
    let read = unsafe {
        heif::heif_context_read_from_memory_without_copy(
            context,
            bytes.as_ptr().cast(),
            bytes.len(),
            ptr::null(),
        )
    };
    assert_eq!(read.code, heif::heif_error_code_heif_error_Ok);
    let mut handle = ptr::null_mut();
    let primary = unsafe { heif::heif_context_get_primary_image_handle(context, &mut handle) };
    assert_eq!(primary.code, heif::heif_error_code_heif_error_Ok);
    assert_eq!(
        unsafe { heif::heif_image_handle_get_width(handle) },
        i32::try_from(width).unwrap()
    );
    assert_eq!(
        unsafe { heif::heif_image_handle_get_height(handle) },
        i32::try_from(height).unwrap()
    );
    assert_eq!(
        unsafe { heif::heif_image_handle_get_luma_bits_per_pixel(handle) },
        depth
    );
    unsafe {
        heif::heif_image_handle_release(handle);
        heif::heif_context_free(context);
    }
}

#[cfg(feature = "heic")]
#[test]
fn heic_collision_and_output_limit_are_transactional() {
    let directory = tempfile::tempdir().unwrap();
    let input = directory.path().join("input.png");
    image::save_buffer_with_format(
        &input,
        &[20_u8, 40, 60],
        1,
        1,
        image::ColorType::Rgb8,
        image::ImageFormat::Png,
    )
    .unwrap();
    let alias = directory.path().join("alias.heic");
    fs::hard_link(&input, &alias).unwrap();
    let collision = Command::new(env!("CARGO_BIN_EXE_grainroom"))
        .args(["develop", "--input"])
        .arg(&input)
        .arg("--output")
        .arg(&alias)
        .args(["--format", "heic", "--overwrite", "--json"])
        .output()
        .unwrap();
    assert_eq!(collision.status.code(), Some(1));
    assert_eq!(json(&collision)["outcome"]["code"], "destination_conflict");
    assert_eq!(fs::read(&alias).unwrap(), fs::read(&input).unwrap());

    let limited = directory.path().join("limited.heic");
    let result = Command::new(env!("CARGO_BIN_EXE_grainroom"))
        .args(["develop", "--input"])
        .arg(&input)
        .arg("--output")
        .arg(&limited)
        .args(["--format", "heic", "--max-output-bytes", "64", "--json"])
        .output()
        .unwrap();
    assert_eq!(result.status.code(), Some(1));
    assert_eq!(json(&result)["outcome"]["code"], "resource_limit");
    assert!(!limited.exists());
    assert!(
        !fs::read_dir(directory.path())
            .unwrap()
            .flatten()
            .any(|entry| entry
                .file_name()
                .to_string_lossy()
                .contains("grainroom-tmp"))
    );
}

#[cfg(feature = "heic")]
#[test]
fn invalid_heic_options_precede_preset_fifo_and_input_io() {
    use rustix::fs::Mode;

    let directory = tempfile::tempdir().unwrap();
    let preset = directory.path().join("preset.json");
    rustix::fs::mkfifoat(rustix::fs::CWD, &preset, Mode::RUSR | Mode::WUSR).unwrap();
    let result = Command::new(env!("CARGO_BIN_EXE_grainroom"))
        .args(["develop", "--input"])
        .arg(directory.path().join("missing.png"))
        .arg("--output")
        .arg(directory.path().join("missing.heic"))
        .arg("--preset-file")
        .arg(&preset)
        .args(["--max-source-bytes", "0"])
        .output()
        .unwrap();
    assert_eq!(result.status.code(), Some(2));
    assert!(preset.exists());
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
