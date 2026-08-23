use super::{
    CommandExit,
    args::{Cli, Command, DevelopArgs, DevelopFormat, ParametersCommand, PresetsCommand},
};
use grainroom::{
    develop::{
        DevelopStage, ParameterKind, ParameterUnit, PresetCatalog, apply_parameter_overrides,
        parameter_registry,
    },
    io::raw::{RawCapability, probe_dcraw_emu},
};
use serde_json::json;
use std::{
    collections::HashSet,
    ffi::OsStr,
    io::{self, Write},
    path::{Path, PathBuf},
    process::{Command as ProcessCommand, ExitStatus},
};

pub(crate) trait GuiResolver {
    fn packaged_sibling(&self) -> io::Result<PathBuf>;
}

pub(crate) trait GuiProcess {
    fn launch(&mut self, executable: &Path, input: Option<&OsStr>) -> io::Result<ExitStatus>;
}

pub(crate) struct SystemGuiResolver;

impl GuiResolver for SystemGuiResolver {
    fn packaged_sibling(&self) -> io::Result<PathBuf> {
        let executable = std::env::current_exe()?;
        let directory = executable
            .parent()
            .ok_or_else(|| io::Error::other("core executable has no parent directory"))?;
        Ok(directory.join("grainroom-gui"))
    }
}

pub(crate) struct SystemGuiProcess;

impl GuiProcess for SystemGuiProcess {
    fn launch(&mut self, executable: &Path, input: Option<&OsStr>) -> io::Result<ExitStatus> {
        let mut command = ProcessCommand::new(executable);
        if let Some(input) = input {
            command.arg("--input").arg(input);
        }
        command.status()
    }
}

pub(crate) fn dispatch(
    cli: Cli,
    resolver: &dyn GuiResolver,
    process: &mut dyn GuiProcess,
    stdout: &mut dyn Write,
    stderr: &mut dyn Write,
) -> CommandExit {
    match cli.command {
        Command::Gui(arguments) => launch_gui(
            resolver,
            process,
            arguments.input.as_deref().map(Path::as_os_str),
            stderr,
        ),
        Command::Develop(arguments) => validate_develop(arguments, stderr),
        Command::Presets(arguments) => match arguments.command {
            PresetsCommand::List => list_presets(stdout, stderr),
            PresetsCommand::Show { id } => show_preset(&id, stdout, stderr),
        },
        Command::Parameters(arguments) => match arguments.command {
            ParametersCommand::List => list_parameters(stdout, stderr),
        },
        Command::Probe => probe(stdout, stderr),
    }
}

fn launch_gui(
    resolver: &dyn GuiResolver,
    process: &mut dyn GuiProcess,
    input: Option<&OsStr>,
    stderr: &mut dyn Write,
) -> CommandExit {
    let executable = match resolver.packaged_sibling() {
        Ok(path) => path,
        Err(_) => {
            human_error(stderr, "packaged GUI sibling could not be resolved");
            return CommandExit::Unavailable;
        }
    };
    match process.launch(&executable, input) {
        Ok(status) => match status.code() {
            Some(code @ 0..=255) => CommandExit::Child(code as u8),
            _ => CommandExit::Internal,
        },
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            human_error(stderr, "packaged GUI sibling is not installed");
            CommandExit::Unavailable
        }
        Err(_) => {
            human_error(stderr, "packaged GUI sibling could not be started");
            CommandExit::Internal
        }
    }
}

fn validate_develop(arguments: DevelopArgs, stderr: &mut dyn Write) -> CommandExit {
    let format = match arguments.format.or_else(|| infer_format(&arguments.output)) {
        Some(format) => format,
        None => {
            human_error(
                stderr,
                "output format is required when the destination suffix is not .jpg, .jpeg, .heic, or .heif",
            );
            return CommandExit::Usage;
        }
    };
    let mut ids = HashSet::new();
    if let Some(duplicate) = arguments
        .overrides
        .iter()
        .map(|value| value.parameter_id())
        .find(|id| !ids.insert(*id))
    {
        human_error(
            stderr,
            &format!("parameter {duplicate:?} is overridden twice"),
        );
        return CommandExit::Usage;
    }
    let catalog = match PresetCatalog::built_in() {
        Ok(catalog) => catalog,
        Err(_) => {
            human_error(stderr, "built-in preset catalog is unavailable");
            return CommandExit::Internal;
        }
    };
    let Some(preset) = catalog.get(&arguments.preset) else {
        human_error(stderr, "unknown built-in preset ID");
        return CommandExit::Usage;
    };
    if apply_parameter_overrides(&preset.settings, &arguments.overrides).is_err() {
        human_error(stderr, "parameter overrides do not form valid settings");
        return CommandExit::Usage;
    }
    let _validated_request = (
        arguments.input,
        arguments.output,
        format,
        arguments.quality,
        arguments.overwrite,
    );
    human_error(
        stderr,
        "develop execution is unavailable until the job and encoder boundary is integrated",
    );
    CommandExit::Unavailable
}

fn infer_format(output: &Path) -> Option<DevelopFormat> {
    let suffix = output.extension()?.to_str()?;
    if suffix.eq_ignore_ascii_case("jpg") || suffix.eq_ignore_ascii_case("jpeg") {
        Some(DevelopFormat::Jpeg)
    } else if suffix.eq_ignore_ascii_case("heic") || suffix.eq_ignore_ascii_case("heif") {
        Some(DevelopFormat::Heic)
    } else {
        None
    }
}

fn list_presets(stdout: &mut dyn Write, stderr: &mut dyn Write) -> CommandExit {
    let catalog = match PresetCatalog::built_in() {
        Ok(catalog) => catalog,
        Err(_) => {
            human_error(stderr, "built-in preset catalog is unavailable");
            return CommandExit::Internal;
        }
    };
    let values = catalog
        .documents()
        .iter()
        .map(|preset| json!({"id": preset.id, "name": preset.name}))
        .collect::<Vec<_>>();
    write_json(stdout, &json!({"presets": values}), stderr)
}

fn show_preset(id: &str, stdout: &mut dyn Write, stderr: &mut dyn Write) -> CommandExit {
    let catalog = match PresetCatalog::built_in() {
        Ok(catalog) => catalog,
        Err(_) => {
            human_error(stderr, "built-in preset catalog is unavailable");
            return CommandExit::Internal;
        }
    };
    let Some(preset) = catalog.get(id) else {
        human_error(stderr, "unknown built-in preset ID");
        return CommandExit::Usage;
    };
    match preset.to_canonical_json() {
        Ok(json) if writeln!(stdout, "{json}").is_ok() => CommandExit::Success,
        _ => {
            human_error(stderr, "preset output could not be written");
            CommandExit::Internal
        }
    }
}

fn list_parameters(stdout: &mut dyn Write, stderr: &mut dyn Write) -> CommandExit {
    let values = parameter_registry()
        .into_iter()
        .map(|parameter| {
            json!({
                "id": parameter.id,
                "label": parameter.label,
                "stage": stage_name(parameter.stage),
                "kind": kind_name(parameter.kind),
                "unit": unit_name(parameter.unit),
                "minimum": parameter.minimum,
                "maximum": parameter.maximum,
                "neutral": parameter.neutral,
                "step": parameter.step,
            })
        })
        .collect::<Vec<_>>();
    write_json(stdout, &json!({"parameters": values}), stderr)
}

fn probe(stdout: &mut dyn Write, stderr: &mut dyn Write) -> CommandExit {
    let available = matches!(probe_dcraw_emu(), RawCapability::Available { .. });
    write_json(
        stdout,
        &json!({"raw": {"available": available, "backend": "libraw-dcraw-emu"}}),
        stderr,
    )
}

fn write_json(
    stdout: &mut dyn Write,
    value: &serde_json::Value,
    stderr: &mut dyn Write,
) -> CommandExit {
    if serde_json::to_writer(&mut *stdout, value).is_ok() && writeln!(stdout).is_ok() {
        CommandExit::Success
    } else {
        human_error(stderr, "JSON output could not be written");
        CommandExit::Internal
    }
}

fn human_error(stderr: &mut dyn Write, message: &str) {
    let _ = writeln!(stderr, "grainroom: {message}");
}

fn stage_name(value: DevelopStage) -> &'static str {
    match value {
        DevelopStage::Geometry => "geometry",
        DevelopStage::Basics => "basics",
        DevelopStage::ToneCurves => "tone_curves",
        DevelopStage::ColorMixer => "color_mixer",
        DevelopStage::ColorGrading => "color_grading",
        DevelopStage::Effects => "effects",
        DevelopStage::RadialMasks => "radial_masks",
    }
}

fn kind_name(value: ParameterKind) -> &'static str {
    match value {
        ParameterKind::Collection => "collection",
        ParameterKind::Scalar => "scalar",
        ParameterKind::Toggle => "toggle",
        ParameterKind::Curve => "curve",
        ParameterKind::Identifier => "identifier",
        ParameterKind::Presence => "presence",
    }
}

fn unit_name(value: ParameterUnit) -> &'static str {
    match value {
        ParameterUnit::Boolean => "boolean",
        ParameterUnit::Bytes => "bytes",
        ParameterUnit::ControlPoints => "control_points",
        ParameterUnit::Degrees => "degrees",
        ParameterUnit::FilmIso => "film_iso",
        ParameterUnit::Items => "items",
        ParameterUnit::Normalized => "normalized",
        ParameterUnit::Percent => "percent",
        ParameterUnit::QuarterTurns => "quarter_turns",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::command::args::{GuiArgs, PresetsArgs};
    use std::{
        cell::RefCell,
        os::unix::{ffi::OsStringExt, process::ExitStatusExt},
    };

    struct Resolver(PathBuf);
    impl GuiResolver for Resolver {
        fn packaged_sibling(&self) -> io::Result<PathBuf> {
            Ok(self.0.clone())
        }
    }

    #[derive(Default)]
    struct Process {
        call: RefCell<Option<(PathBuf, Option<std::ffi::OsString>)>>,
    }
    impl GuiProcess for Process {
        fn launch(&mut self, executable: &Path, input: Option<&OsStr>) -> io::Result<ExitStatus> {
            self.call.replace(Some((
                executable.to_owned(),
                input.map(OsStr::to_os_string),
            )));
            Ok(ExitStatus::from_raw(0))
        }
    }

    #[test]
    fn gui_uses_only_resolved_sibling_and_preserves_os_input() {
        let sibling = PathBuf::from("/packaged/bin/grainroom-gui");
        let input = PathBuf::from(std::ffi::OsString::from_vec(b"photo-\xff.jpg".to_vec()));
        let mut process = Process::default();
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        let exit = dispatch(
            Cli {
                command: Command::Gui(GuiArgs {
                    input: Some(input.clone()),
                }),
            },
            &Resolver(sibling.clone()),
            &mut process,
            &mut stdout,
            &mut stderr,
        );
        assert_eq!(exit, CommandExit::Child(0));
        assert_eq!(
            process.call.into_inner(),
            Some((sibling, Some(input.into_os_string())))
        );
    }

    #[test]
    fn preset_json_contains_no_paths() {
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        let exit = dispatch(
            Cli {
                command: Command::Presets(PresetsArgs {
                    command: PresetsCommand::List,
                }),
            },
            &Resolver(PathBuf::new()),
            &mut Process::default(),
            &mut stdout,
            &mut stderr,
        );
        assert_eq!(exit, CommandExit::Success);
        let output: serde_json::Value = serde_json::from_slice(&stdout).unwrap();
        assert_eq!(output["presets"][0]["id"], "neutral");
        assert!(!String::from_utf8(stdout).unwrap().contains('/'));
    }
}
