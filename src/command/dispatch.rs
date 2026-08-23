use super::{
    CommandExit,
    args::{Cli, Command, DevelopArgs, DevelopFormat, ParametersCommand, PresetsCommand},
};
use grainroom::develop::{
    DevelopStage, ParameterKind, ParameterUnit, PresetCatalog, apply_parameter_overrides,
    parameter_registry,
};
use serde_json::json;
use std::{
    collections::HashSet,
    ffi::OsStr,
    fs::File,
    io::{self, Read, Write},
    os::{
        fd::AsRawFd,
        unix::{
            fs::{MetadataExt, PermissionsExt},
            process::CommandExt,
        },
    },
    path::{Path, PathBuf},
    process::{Command as ProcessCommand, ExitStatus, Stdio},
    time::{Duration, Instant},
};

const RAW_PROBE_CANDIDATES: [&str; 2] = ["/usr/bin/dcraw_emu", "/usr/local/bin/dcraw_emu"];
const RAW_PROBE_SENTINEL: &str = "/proc/self/fd/2147483647";
const RAW_PROBE_CAPTURE_BYTES: usize = 8 * 1024;

pub(crate) trait GuiResolver {
    fn packaged_sibling(&self) -> io::Result<HeldGuiExecutable>;
}

pub(crate) trait GuiProcess {
    fn launch(
        &mut self,
        executable: &HeldGuiExecutable,
        input: Option<&OsStr>,
    ) -> io::Result<ExitStatus>;
}

pub(crate) struct SystemGuiResolver;

pub(crate) struct HeldGuiExecutable {
    file: File,
}

impl HeldGuiExecutable {
    #[cfg(target_os = "linux")]
    fn proc_path(&self) -> PathBuf {
        PathBuf::from(format!("/proc/self/fd/{}", self.file.as_raw_fd()))
    }
}

impl GuiResolver for SystemGuiResolver {
    fn packaged_sibling(&self) -> io::Result<HeldGuiExecutable> {
        let executable = std::env::current_exe()?;
        resolve_gui_sibling(&executable)
    }
}

#[cfg(target_os = "linux")]
fn resolve_gui_sibling(core_executable: &Path) -> io::Result<HeldGuiExecutable> {
    use rustix::fs::{self, FileType, Mode, OFlags};

    let directory = core_executable
        .parent()
        .ok_or_else(|| io::Error::other("core executable has no parent directory"))?;
    let directory = fs::open(
        directory,
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::empty(),
    )
    .map_err(io::Error::from)?;
    let sibling = fs::openat(
        &directory,
        "grainroom-gui",
        // The descriptor deliberately survives exec: script interpreters and
        // `/proc/self/fd` launch both need the held object after pathname
        // resolution. The GUI receives only this read-only executable fd.
        OFlags::RDONLY | OFlags::NOFOLLOW,
        Mode::empty(),
    )
    .map_err(io::Error::from)?;
    let metadata = fs::fstat(&sibling).map_err(io::Error::from)?;
    if FileType::from_raw_mode(metadata.st_mode) != FileType::RegularFile
        || metadata.st_mode & 0o111 == 0
    {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "packaged GUI sibling is not a regular executable",
        ));
    }
    Ok(HeldGuiExecutable {
        file: File::from(sibling),
    })
}

#[cfg(not(target_os = "linux"))]
fn resolve_gui_sibling(_core_executable: &Path) -> io::Result<HeldGuiExecutable> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "secure packaged GUI launch is unavailable on this platform",
    ))
}

pub(crate) struct SystemGuiProcess;

impl GuiProcess for SystemGuiProcess {
    fn launch(
        &mut self,
        executable: &HeldGuiExecutable,
        input: Option<&OsStr>,
    ) -> io::Result<ExitStatus> {
        #[cfg(not(target_os = "linux"))]
        return Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "secure packaged GUI launch is unavailable on this platform",
        ));
        #[cfg(target_os = "linux")]
        let mut command = ProcessCommand::new(executable.proc_path());
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
        Command::Develop(arguments) => validate_develop(arguments, stdout, stderr),
        Command::Presets(arguments) => match arguments.command {
            PresetsCommand::List { json } => list_presets(json, stdout, stderr),
            PresetsCommand::Show { id, json } => show_preset(&id, json, stdout, stderr),
        },
        Command::Parameters(arguments) => match arguments.command {
            ParametersCommand::List { json } => list_parameters(json, stdout, stderr),
        },
        Command::Probe(arguments) => probe(arguments.json, stdout, stderr),
    }
}

fn launch_gui(
    resolver: &dyn GuiResolver,
    process: &mut dyn GuiProcess,
    input: Option<&OsStr>,
    stderr: &mut dyn Write,
) -> CommandExit {
    let executable = match resolver.packaged_sibling() {
        Ok(executable) => executable,
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

fn validate_develop(
    arguments: DevelopArgs,
    stdout: &mut dyn Write,
    stderr: &mut dyn Write,
) -> CommandExit {
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
    if arguments.preset_file.is_none() {
        let catalog = match PresetCatalog::built_in() {
            Ok(catalog) => catalog,
            Err(_) => {
                human_error(stderr, "built-in preset catalog is unavailable");
                return CommandExit::Internal;
            }
        };
        let preset_id = arguments.preset.as_deref().unwrap_or("neutral");
        let Some(preset) = catalog.get(preset_id) else {
            human_error(stderr, "unknown built-in preset ID");
            return CommandExit::Usage;
        };
        if apply_parameter_overrides(&preset.settings, &arguments.overrides).is_err() {
            human_error(stderr, "parameter overrides do not form valid settings");
            return CommandExit::Usage;
        }
    }
    let _validated_request = (
        arguments.input,
        arguments.output,
        format,
        arguments.quality,
        arguments.preset_file,
        arguments.unprofiled,
        arguments.metadata,
        arguments.alpha,
        arguments.overwrite,
        arguments.progress,
    );
    if arguments.json {
        if write_json(
            stdout,
            &json!({"error": {"code": "unavailable", "message": "develop execution is unavailable"}}),
            stderr,
        ) != CommandExit::Success
        {
            return CommandExit::Internal;
        }
    } else {
        human_error(
            stderr,
            "develop execution is unavailable until the job and encoder boundary is integrated",
        );
    }
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

fn list_presets(json_output: bool, stdout: &mut dyn Write, stderr: &mut dyn Write) -> CommandExit {
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
    if json_output {
        write_json(stdout, &json!({"presets": values}), stderr)
    } else if catalog
        .documents()
        .iter()
        .all(|preset| writeln!(stdout, "{}\t{}", preset.id, preset.name).is_ok())
    {
        CommandExit::Success
    } else {
        human_error(stderr, "preset output could not be written");
        CommandExit::Internal
    }
}

fn show_preset(
    id: &str,
    json_output: bool,
    stdout: &mut dyn Write,
    stderr: &mut dyn Write,
) -> CommandExit {
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
    let output = if json_output {
        preset.to_canonical_json()
    } else {
        serde_json::to_string_pretty(preset).map_err(grainroom::develop::PresetError::Json)
    };
    match output {
        Ok(output) if writeln!(stdout, "{output}").is_ok() => CommandExit::Success,
        _ => {
            human_error(stderr, "preset output could not be written");
            CommandExit::Internal
        }
    }
}

fn list_parameters(
    json_output: bool,
    stdout: &mut dyn Write,
    stderr: &mut dyn Write,
) -> CommandExit {
    let registry = parameter_registry();
    let values = registry
        .iter()
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
    if json_output {
        write_json(stdout, &json!({"parameters": values}), stderr)
    } else if registry.iter().all(|parameter| {
        writeln!(
            stdout,
            "{}\t{}\t{}",
            parameter.id,
            kind_name(parameter.kind),
            parameter.label
        )
        .is_ok()
    }) {
        CommandExit::Success
    } else {
        human_error(stderr, "parameter output could not be written");
        CommandExit::Internal
    }
}

fn probe(json_output: bool, stdout: &mut dyn Write, stderr: &mut dyn Write) -> CommandExit {
    let available = trusted_raw_probe(&RAW_PROBE_CANDIDATES);
    if json_output {
        write_json(
            stdout,
            &json!({"raw": {"available": available, "backend": "libraw-dcraw-emu"}}),
            stderr,
        )
    } else if writeln!(
        stdout,
        "raw\tlibraw-dcraw-emu\t{}",
        if available {
            "available"
        } else {
            "unavailable"
        }
    )
    .is_ok()
    {
        CommandExit::Success
    } else {
        human_error(stderr, "probe output could not be written");
        CommandExit::Internal
    }
}

fn trusted_raw_probe(candidates: &[&str]) -> bool {
    candidates.iter().any(|candidate| {
        let path = Path::new(candidate);
        let Ok(metadata) = std::fs::symlink_metadata(path) else {
            return false;
        };
        path.is_absolute()
            && metadata.file_type().is_file()
            && metadata.uid() == 0
            && metadata.permissions().mode() & 0o111 != 0
            && metadata.permissions().mode() & 0o022 == 0
            && functional_dcraw_handshake(path)
    })
}

fn functional_dcraw_handshake(executable: &Path) -> bool {
    let mut command = ProcessCommand::new(executable);
    command
        .arg("-v")
        .arg(RAW_PROBE_SENTINEL)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .process_group(0);
    let mut child = match command.spawn() {
        Ok(child) => child,
        Err(_) => return false,
    };
    let process_group = child.id();
    let Some(leader) = rustix::process::Pid::from_raw(process_group as i32) else {
        kill_probe_group(process_group);
        let _ = child.wait();
        return false;
    };
    let (Some(mut stdout), Some(mut stderr)) = (child.stdout.take(), child.stderr.take()) else {
        kill_probe_group(process_group);
        let _ = child.wait();
        return false;
    };
    if !set_nonblocking(&stdout) || !set_nonblocking(&stderr) {
        kill_probe_group(process_group);
        let _ = child.wait();
        return false;
    }
    let deadline = Instant::now() + Duration::from_millis(750);
    let mut observed_exit = None;
    let mut stdout_eof = false;
    let mut stderr_eof = false;
    let mut stdout_bytes = Vec::new();
    let mut stderr_bytes = Vec::new();
    let completed = loop {
        if !drain_probe_stream(&mut stdout, &mut stdout_bytes, &mut stdout_eof)
            || !drain_probe_stream(&mut stderr, &mut stderr_bytes, &mut stderr_eof)
        {
            break false;
        }
        if observed_exit.is_none() {
            match observe_probe_exit(leader) {
                Ok(value) => observed_exit = value,
                Err(()) => break false,
            }
        }
        if observed_exit.is_some() && stdout_eof && stderr_eof {
            break true;
        }
        if Instant::now() >= deadline {
            break false;
        }
        std::thread::sleep(Duration::from_millis(5));
    };
    if !completed {
        kill_probe_group(process_group);
        let _ = child.wait();
        return false;
    }
    let Some(exit_code) = observed_exit else {
        kill_probe_group(process_group);
        let _ = child.wait();
        return false;
    };
    // Keep the exited leader unreaped until all same-group survivors are
    // terminated. This pins the numeric PID/PGID against reuse.
    if observe_probe_exit(leader).ok().flatten().is_none() {
        kill_probe_group(process_group);
        let _ = child.wait();
        return false;
    }
    kill_probe_group(process_group);
    std::thread::sleep(Duration::from_millis(10));
    if observe_probe_exit(leader).ok().flatten().is_none() {
        let _ = child.wait();
        return false;
    }
    let status = match child.wait() {
        Ok(status) => status,
        Err(_) => return false,
    };
    if status.code() != Some(exit_code) {
        return false;
    }
    let mut output = stdout_bytes;
    output.extend_from_slice(&stderr_bytes);
    let output = String::from_utf8_lossy(&output);
    exit_code == 2
        && output.contains("Using ")
        && output.contains(" threads")
        && output.contains(&format!("Processing file {RAW_PROBE_SENTINEL}"))
        && output.contains("Cannot open")
}

fn set_nonblocking(file: &impl std::os::fd::AsFd) -> bool {
    let Ok(flags) = rustix::fs::fcntl_getfl(file) else {
        return false;
    };
    rustix::fs::fcntl_setfl(file, flags | rustix::fs::OFlags::NONBLOCK).is_ok()
}

fn drain_probe_stream(stream: &mut impl Read, retained: &mut Vec<u8>, eof: &mut bool) -> bool {
    let mut chunk = [0_u8; 1024];
    loop {
        match stream.read(&mut chunk) {
            Ok(0) => {
                *eof = true;
                return true;
            }
            Ok(count) => {
                let Some(new_len) = retained.len().checked_add(count) else {
                    return false;
                };
                if new_len > RAW_PROBE_CAPTURE_BYTES {
                    return false;
                }
                retained.extend_from_slice(&chunk[..count]);
            }
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => return true,
            Err(_) => return false,
        }
    }
}

fn observe_probe_exit(leader: rustix::process::Pid) -> Result<Option<i32>, ()> {
    rustix::process::waitid(
        rustix::process::WaitId::Pid(leader),
        rustix::process::WaitIdOptions::EXITED
            | rustix::process::WaitIdOptions::NOHANG
            | rustix::process::WaitIdOptions::NOWAIT,
    )
    .map(|status| status.and_then(|status| status.exit_status()))
    .map_err(|_| ())
}

fn kill_probe_group(process_group: u32) {
    if let Some(pid) = rustix::process::Pid::from_raw(process_group as i32) {
        let _ = rustix::process::kill_process_group(pid, rustix::process::Signal::KILL);
    }
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
        fs,
        os::unix::{ffi::OsStringExt, fs::PermissionsExt, process::ExitStatusExt},
    };
    use tempfile::tempdir;

    struct Resolver(File);
    impl GuiResolver for Resolver {
        fn packaged_sibling(&self) -> io::Result<HeldGuiExecutable> {
            Ok(HeldGuiExecutable {
                file: self.0.try_clone()?,
            })
        }
    }

    #[derive(Default)]
    struct Process {
        call: RefCell<Option<(bool, Option<std::ffi::OsString>)>>,
    }
    impl GuiProcess for Process {
        fn launch(
            &mut self,
            executable: &HeldGuiExecutable,
            input: Option<&OsStr>,
        ) -> io::Result<ExitStatus> {
            self.call.replace(Some((
                executable.file.metadata()?.is_file(),
                input.map(OsStr::to_os_string),
            )));
            Ok(ExitStatus::from_raw(0))
        }
    }

    #[test]
    fn gui_uses_only_resolved_sibling_and_preserves_os_input() {
        let directory = tempdir().unwrap();
        let sibling = directory.path().join("grainroom-gui");
        fs::write(&sibling, b"held executable").unwrap();
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
            &Resolver(File::open(sibling).unwrap()),
            &mut process,
            &mut stdout,
            &mut stderr,
        );
        assert_eq!(exit, CommandExit::Child(0));
        assert_eq!(
            process.call.into_inner(),
            Some((true, Some(input.into_os_string())))
        );
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn secure_gui_resolution_rejects_symlinks_and_non_executables() {
        let directory = tempdir().unwrap();
        let core = directory.path().join("grainroom");
        fs::write(&core, b"core").unwrap();
        let sibling = directory.path().join("grainroom-gui");
        std::os::unix::fs::symlink("/bin/true", &sibling).unwrap();
        assert!(resolve_gui_sibling(&core).is_err());

        fs::remove_file(&sibling).unwrap();
        fs::write(&sibling, b"not executable").unwrap();
        assert!(resolve_gui_sibling(&core).is_err());

        fs::set_permissions(&sibling, fs::Permissions::from_mode(0o700)).unwrap();
        let held = resolve_gui_sibling(&core).unwrap();
        fs::rename(&sibling, directory.path().join("replaced")).unwrap();
        fs::write(&sibling, b"replacement").unwrap();
        assert_eq!(fs::read(held.proc_path()).unwrap(), b"not executable");
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn system_gui_process_executes_the_held_file() {
        let directory = tempdir().unwrap();
        let core = directory.path().join("grainroom");
        fs::write(&core, b"core").unwrap();
        let sibling = directory.path().join("grainroom-gui");
        fs::write(&sibling, b"#!/bin/sh\nexit 23\n").unwrap();
        fs::set_permissions(&sibling, fs::Permissions::from_mode(0o700)).unwrap();
        let held = resolve_gui_sibling(&core).unwrap();
        fs::rename(&sibling, directory.path().join("old-gui")).unwrap();
        fs::write(&sibling, b"#!/bin/sh\nexit 47\n").unwrap();
        fs::set_permissions(&sibling, fs::Permissions::from_mode(0o700)).unwrap();
        assert_eq!(
            SystemGuiProcess.launch(&held, None).unwrap().code(),
            Some(23)
        );
    }

    #[test]
    fn raw_probe_requires_the_expected_bounded_behavior() {
        let directory = tempdir().unwrap();
        let valid = directory.path().join("valid");
        fs::write(
            &valid,
            format!(
                "#!/bin/sh\nprintf 'Using 4 threads\\nProcessing file {RAW_PROBE_SENTINEL}\\nCannot open {RAW_PROBE_SENTINEL}: Input/output error\\n' >&2\nexit 2\n"
            ),
        )
        .unwrap();
        fs::set_permissions(&valid, fs::Permissions::from_mode(0o700)).unwrap();
        assert!(functional_dcraw_handshake(&valid));

        let impostor = directory.path().join("dcraw_emu");
        fs::write(&impostor, "#!/bin/sh\nexit 0\n").unwrap();
        fs::set_permissions(&impostor, fs::Permissions::from_mode(0o700)).unwrap();
        assert!(!functional_dcraw_handshake(&impostor));
        assert!(!trusted_raw_probe(&[impostor.to_str().unwrap()]));

        let survivor_pid = directory.path().join("survivor-pid");
        let same_group = directory.path().join("same-group");
        fs::write(
            &same_group,
            format!(
                "#!/bin/sh\nprintf 'Using 4 threads\\nProcessing file {RAW_PROBE_SENTINEL}\\nCannot open {RAW_PROBE_SENTINEL}\\n'\nsleep 5 >/dev/null 2>&1 & echo $! > '{}'\nexit 2\n",
                survivor_pid.display()
            ),
        )
        .unwrap();
        fs::set_permissions(&same_group, fs::Permissions::from_mode(0o700)).unwrap();
        assert!(functional_dcraw_handshake(&same_group));
        assert_not_running(fs::read_to_string(&survivor_pid).unwrap().trim());

        let detached_pid = directory.path().join("detached-pid");
        let detached = directory.path().join("detached");
        fs::write(
            &detached,
            format!(
                "#!/bin/sh\nsetsid sh -c 'echo $$ > \"{}\"; sleep 5' &\nexit 0\n",
                detached_pid.display()
            ),
        )
        .unwrap();
        fs::set_permissions(&detached, fs::Permissions::from_mode(0o700)).unwrap();
        let started = Instant::now();
        assert!(!functional_dcraw_handshake(&detached));
        assert!(started.elapsed() < Duration::from_millis(1_200));
        if let Ok(pid) = fs::read_to_string(detached_pid)
            && let Ok(pid) = pid.trim().parse::<i32>()
            && let Some(pid) = rustix::process::Pid::from_raw(pid)
        {
            let _ = rustix::process::kill_process(pid, rustix::process::Signal::KILL);
        }
    }

    fn assert_not_running(pid: &str) {
        let status = fs::read_to_string(format!("/proc/{pid}/stat"));
        if let Ok(status) = status {
            let state = status.rsplit_once(") ").unwrap().1.as_bytes()[0];
            assert_eq!(state, b'Z', "probe survivor {pid} is still running");
        }
    }

    #[test]
    fn preset_json_contains_no_paths() {
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        let exit = dispatch(
            Cli {
                command: Command::Presets(PresetsArgs {
                    command: PresetsCommand::List { json: true },
                }),
            },
            &Resolver(File::open("/dev/null").unwrap()),
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
