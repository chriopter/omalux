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
    fn unavailable_develop_emits_no_progress_stream() {
        let base = [
            "grainroom",
            "develop",
            "--input",
            "source.raw",
            "--output",
            "result.jpg",
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
    fn list_help_describes_default_tsv_output() {
        let (code, stdout, stderr) = invoke(&["grainroom", "presets", "list", "--help"]);
        assert_eq!(code, ExitCode::SUCCESS);
        assert!(stdout.contains("TSV, or JSON"));
        assert!(stderr.is_empty());
    }
}
