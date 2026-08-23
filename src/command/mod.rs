mod args;
mod dispatch;
mod exit;

use clap::Parser;
use std::{ffi::OsString, io, process::ExitCode};

pub(crate) use args::Cli;
use dispatch::{SystemGuiProcess, SystemGuiResolver, dispatch};
pub(crate) use exit::CommandExit;

pub(crate) fn run() -> ExitCode {
    run_from(std::env::args_os(), &mut io::stdout(), &mut io::stderr())
}

fn run_from(
    arguments: impl IntoIterator<Item = OsString>,
    stdout: &mut dyn io::Write,
    stderr: &mut dyn io::Write,
) -> ExitCode {
    let cli = match Cli::try_parse_from(arguments) {
        Ok(cli) => cli,
        Err(error) => {
            let _ = if error.use_stderr() {
                write!(stderr, "{error}")
            } else {
                write!(stdout, "{error}")
            };
            return CommandExit::from_clap(error.exit_code()).into();
        }
    };
    dispatch(
        cli,
        &SystemGuiResolver,
        &mut SystemGuiProcess,
        stdout,
        stderr,
    )
    .into()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn invoke(arguments: &[&str]) -> (ExitCode, String, String) {
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        let exit = run_from(
            arguments.iter().map(OsString::from),
            &mut stdout,
            &mut stderr,
        );
        (
            exit,
            String::from_utf8(stdout).unwrap(),
            String::from_utf8(stderr).unwrap(),
        )
    }

    #[test]
    fn help_version_and_usage_errors_stop_before_dispatch() {
        let (code, stdout, stderr) = invoke(&["grainroom", "--help"]);
        assert_eq!(code, ExitCode::SUCCESS);
        assert!(stdout.contains("Qt-free photo development tools"));
        assert!(stderr.is_empty());

        let (code, stdout, stderr) = invoke(&["grainroom", "--version"]);
        assert_eq!(code, ExitCode::SUCCESS);
        assert!(stdout.starts_with("grainroom "));
        assert!(stderr.is_empty());

        let (code, stdout, stderr) = invoke(&["grainroom", "--headless"]);
        assert_eq!(code, ExitCode::from(2));
        assert!(stdout.is_empty());
        assert!(stderr.contains("unexpected argument '--headless'"));
    }

    #[test]
    fn unavailable_heic_emits_no_progress_stream_or_file_io() {
        let base = [
            "grainroom",
            "develop",
            "--input",
            "source.raw",
            "--output",
            "result.heic",
        ];
        let mut human = base.to_vec();
        human.extend(["--progress", "human"]);
        let (code, stdout, stderr) = invoke(&human);
        assert_eq!(code, ExitCode::from(69));
        assert!(stdout.is_empty());
        assert_eq!(stderr.lines().count(), 1);

        let mut json = base.to_vec();
        json.extend(["--progress", "json", "--json"]);
        let (code, stdout, stderr) = invoke(&json);
        assert_eq!(code, ExitCode::from(69));
        assert!(stderr.is_empty());
        let value: serde_json::Value = serde_json::from_str(&stdout).unwrap();
        assert_eq!(value["error"]["code"], "unavailable");
        assert_eq!(stdout.lines().count(), 1);
    }

    #[test]
    fn neutral_jpeg_runs_end_to_end_with_path_free_json_progress() {
        let directory = tempfile::tempdir().unwrap();
        let input = directory.path().join("input.jpg");
        let output = directory.path().join("output.jpg");
        image::save_buffer_with_format(
            &input,
            &[20_u8, 80, 160],
            1,
            1,
            image::ColorType::Rgb8,
            image::ImageFormat::Jpeg,
        )
        .unwrap();
        let arguments = vec![
            OsString::from("grainroom"),
            OsString::from("develop"),
            OsString::from("--input"),
            input.clone().into_os_string(),
            OsString::from("--output"),
            output.clone().into_os_string(),
            OsString::from("--json"),
            OsString::from("--progress"),
            OsString::from("json"),
        ];
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        let exit = run_from(arguments, &mut stdout, &mut stderr);
        assert_eq!(exit, ExitCode::SUCCESS);
        assert!(output.is_file());
        let report: serde_json::Value = serde_json::from_slice(&stdout).unwrap();
        assert_eq!(report["outcome"]["status"], "published_and_durable");
        let progress = String::from_utf8(stderr).unwrap();
        assert!(progress.lines().count() >= 6);
        let combined = format!("{}{}", String::from_utf8(stdout).unwrap(), progress);
        assert!(!combined.contains(directory.path().to_string_lossy().as_ref()));
    }

    #[test]
    fn hardlink_destination_is_rejected_without_overwriting_source() {
        let directory = tempfile::tempdir().unwrap();
        let input = directory.path().join("input.jpg");
        let output = directory.path().join("output.jpg");
        image::save_buffer_with_format(
            &input,
            &[40_u8, 50, 60],
            1,
            1,
            image::ColorType::Rgb8,
            image::ImageFormat::Jpeg,
        )
        .unwrap();
        std::fs::hard_link(&input, &output).unwrap();
        let before = std::fs::read(&input).unwrap();
        let arguments = vec![
            OsString::from("grainroom"),
            OsString::from("develop"),
            OsString::from("--input"),
            input.into_os_string(),
            OsString::from("--output"),
            output.into_os_string(),
            OsString::from("--overwrite"),
            OsString::from("--json"),
        ];
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        let exit = run_from(arguments, &mut stdout, &mut stderr);
        assert_eq!(exit, ExitCode::from(1));
        let report: serde_json::Value = serde_json::from_slice(&stdout).unwrap();
        assert_eq!(report["outcome"]["code"], "destination_conflict");
        assert_eq!(
            std::fs::read(directory.path().join("input.jpg")).unwrap(),
            before
        );
        assert!(stderr.is_empty());
    }

    #[test]
    fn active_override_stays_honestly_unproven_and_creates_no_target() {
        let directory = tempfile::tempdir().unwrap();
        let input = directory.path().join("input.jpg");
        let output = directory.path().join("output.jpg");
        image::save_buffer_with_format(
            &input,
            &[70_u8, 80, 90],
            1,
            1,
            image::ColorType::Rgb8,
            image::ImageFormat::Jpeg,
        )
        .unwrap();
        let arguments = vec![
            OsString::from("grainroom"),
            OsString::from("develop"),
            OsString::from("--input"),
            input.into_os_string(),
            OsString::from("--output"),
            output.clone().into_os_string(),
            OsString::from("--set"),
            OsString::from("basics.contrast=10"),
            OsString::from("--json"),
        ];
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        let exit = run_from(arguments, &mut stdout, &mut stderr);
        assert_eq!(exit, ExitCode::from(69));
        let report: serde_json::Value = serde_json::from_slice(&stdout).unwrap();
        assert_eq!(report["outcome"]["code"], "unproven_pipeline_budget");
        assert!(!output.exists());
        assert!(stderr.is_empty());
    }

    #[test]
    fn list_help_describes_default_tsv_output() {
        let (code, stdout, stderr) = invoke(&["grainroom", "presets", "list", "--help"]);
        assert_eq!(code, ExitCode::SUCCESS);
        assert!(stdout.contains("TSV, or JSON"));
        assert!(stderr.is_empty());
    }
}
