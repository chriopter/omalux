use super::{
    CommandExit,
    args::{
        AlphaArg, Cli, Command, DevelopArgs, DevelopFormat, MetadataArg, ParametersCommand,
        PresetsCommand, ProgressArg, UnprofiledArg,
    },
};
use omalux::develop::{
    DevelopStage, ParameterKind, ParameterOverrideError, ParameterUnit, PresetCatalog,
    apply_parameter_overrides, load_preset_file, parameter_registry,
};
use omalux::{
    io::{
        AlphaPolicy, DecodeOptions, MetadataPolicy, OutputFormat, OutputProfile, OverwritePolicy,
        ResourceLimits, SdrRangePolicy, UnprofiledPolicy,
    },
    job::{
        CancellationToken, DevelopJob, DevelopJobOutcome, DevelopJobRunner, DevelopOutput,
        JobErrorCode, JobStage, PresetSelection, ProductionPhotoDecoder, ProductionPhotoEncoder,
        ProgressSink,
    },
};
use serde_json::json;
use std::{
    ffi::OsStr,
    fs::File,
    io::{self, Write},
    os::fd::AsRawFd,
    path::{Path, PathBuf},
    process::{Command as ProcessCommand, ExitStatus},
    sync::{
        Mutex, OnceLock,
        atomic::{AtomicU64, Ordering},
    },
};

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
        "omalux-gui",
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
    if let Some((_, duplicate)) = arguments
        .overrides
        .iter()
        .enumerate()
        .find(|(index, value)| {
            arguments.overrides[..*index]
                .iter()
                .any(|previous| previous.parameter_id() == value.parameter_id())
        })
    {
        human_error(
            stderr,
            &format!(
                "parameter {:?} is overridden twice",
                duplicate.parameter_id()
            ),
        );
        return CommandExit::Usage;
    }
    // All option-only contracts precede external preset I/O. A malformed
    // resource budget must not touch even a FIFO or inaccessible preset path.
    let mut limits = ResourceLimits::default();
    if let Some(value) = arguments.max_source_bytes {
        limits.max_source_bytes = value;
    }
    if let Some(value) = arguments.max_pixels {
        limits.max_pixels = value;
    }
    if let Some(value) = arguments.max_working_bytes {
        limits.max_working_bytes = value;
    }
    if let Some(value) = arguments.max_output_bytes {
        limits.max_output_bytes = value;
    }
    let mut decode = DecodeOptions::default();
    decode.limits = limits;
    decode.unprofiled = match arguments.unprofiled {
        UnprofiledArg::AssumeSrgb => UnprofiledPolicy::AssumeSrgbAndWarn,
        UnprofiledArg::Reject => UnprofiledPolicy::Reject,
    };
    decode.raw.auto_tone = match arguments.raw_tone {
        crate::command::args::RawToneArg::Auto => true,
        crate::command::args::RawToneArg::Linear => false,
    };
    let alpha = match arguments.alpha {
        AlphaArg::Reject => AlphaPolicy::Reject,
        AlphaArg::FlattenBlack => AlphaPolicy::Flatten([0.0; 3]),
        AlphaArg::Flatten(rgb) => AlphaPolicy::Flatten(rgb.map(|value| f32::from(value) / 255.0)),
    };
    let output_format = match format {
        DevelopFormat::Jpeg => OutputFormat::Jpeg,
        DevelopFormat::Heic => OutputFormat::Heic,
    };
    let output_options = DevelopOutput::new(
        output_format,
        arguments.quality,
        OutputProfile::Srgb,
        match arguments.metadata {
            MetadataArg::PreserveSafe => MetadataPolicy::PreserveSafe,
            MetadataArg::StripLocation => MetadataPolicy::StripLocation,
            MetadataArg::StripAll => MetadataPolicy::StripAll,
        },
        alpha,
        SdrRangePolicy::ClipAndReport,
    );
    if decode.validate().is_err() || output_options.validate().is_err() {
        human_error(stderr, "develop options are invalid");
        return CommandExit::Usage;
    }

    // A binary built without the optional backend rejects otherwise-valid
    // HEIC options before preset/input I/O or destination creation.
    #[cfg(not(feature = "heic"))]
    if format == DevelopFormat::Heic {
        return emit_unavailable(arguments.json, stdout, stderr, "heic_encoder");
    }

    // Capability probing is option-only and deliberately precedes any
    // external preset/input access. Production HEIC v1 is a 10-bit x265 path.
    #[cfg(feature = "heic")]
    if format == DevelopFormat::Heic
        && !matches!(
            omalux::io::probe_heic_capability(),
            Ok(capability) if capability.ten_bit
        )
    {
        return emit_unavailable(arguments.json, stdout, stderr, "heic_encoder");
    }

    let catalog = match PresetCatalog::built_in() {
        Ok(catalog) => catalog,
        Err(_) => {
            human_error(stderr, "built-in preset catalog is unavailable");
            return CommandExit::Internal;
        }
    };
    let preset = if let Some(path) = arguments.preset_file.as_deref() {
        match load_preset_file(path) {
            Ok(document) => PresetSelection::document(document),
            Err(_) => {
                human_error(stderr, "external preset could not be loaded");
                return CommandExit::Usage;
            }
        }
    } else {
        let id = arguments.preset.as_deref().unwrap_or("neutral");
        let Some(document) = catalog.get(id) else {
            human_error(stderr, "unknown built-in preset ID");
            return CommandExit::Usage;
        };
        PresetSelection::CatalogId(document.id.clone())
    };
    let base_settings = match &preset {
        PresetSelection::CatalogId(id) => {
            &catalog.get(id).expect("catalog ID was checked").settings
        }
        PresetSelection::Document(document) => &document.settings,
    };
    if let Err(error) = apply_parameter_overrides(base_settings, &arguments.overrides) {
        if matches!(error, ParameterOverrideError::Allocation) {
            human_error(stderr, "parameter override resources are unavailable");
            return CommandExit::Unavailable;
        }
        human_error(stderr, "parameter overrides do not form valid settings");
        return CommandExit::Usage;
    }

    let job = DevelopJob {
        input: arguments.input,
        output: arguments.output,
        decode,
        output_options,
        overwrite: if arguments.overwrite {
            OverwritePolicy::Replace
        } else {
            OverwritePolicy::Forbid
        },
        preset,
        overrides: arguments.overrides,
    };
    let runner = DevelopJobRunner::new(catalog);
    let decoder = ProductionPhotoDecoder::new();
    let encoder = ProductionPhotoEncoder::new(limits);
    let cancellation = CancellationToken::new();
    let _signal_guard = match ActiveCancellation::install(cancellation.clone()) {
        Ok(guard) => guard,
        Err(()) => {
            human_error(stderr, "interrupt handler could not be installed");
            return CommandExit::Internal;
        }
    };
    let mut progress = CliProgress {
        mode: arguments.progress,
        output: stderr,
    };
    let result = runner.run(&job, &decoder, &encoder, &cancellation, &mut progress);
    match result {
        Ok(report) => {
            if arguments.json {
                write_json(stdout, &report, stderr)
            } else if writeln!(stdout, "{}", human_success(&report.outcome)).is_ok() {
                CommandExit::Success
            } else {
                CommandExit::Internal
            }
        }
        Err(failure) => {
            let exit = failure_exit(failure.error.code);
            if arguments.json {
                if write_json(stdout, &failure.report, stderr) == CommandExit::Success {
                    exit
                } else {
                    CommandExit::Internal
                }
            } else {
                human_error(
                    stderr,
                    &format!(
                        "develop failed at {} ({})",
                        job_stage_name(failure.error.stage),
                        error_name(failure.error.code)
                    ),
                );
                exit
            }
        }
    }
}

fn human_success(outcome: &DevelopJobOutcome) -> String {
    match outcome {
        DevelopJobOutcome::PublishedAndDurable { bytes_written } => {
            format!("develop complete: published_and_durable ({bytes_written} bytes)")
        }
        DevelopJobOutcome::PublishedButNotDurable { bytes_written } => {
            format!("develop complete: published_but_not_durable ({bytes_written} bytes)")
        }
        DevelopJobOutcome::Failure { .. } => "develop failed".to_owned(),
    }
}

static ACTIVE_CANCELLATION: OnceLock<Mutex<Option<(u64, CancellationToken)>>> = OnceLock::new();
static INTERRUPT_HANDLER: OnceLock<Result<(), ()>> = OnceLock::new();
static NEXT_CANCELLATION_ID: AtomicU64 = AtomicU64::new(1);

struct ActiveCancellation {
    id: u64,
}

impl ActiveCancellation {
    fn install(token: CancellationToken) -> Result<Self, ()> {
        let slot = ACTIVE_CANCELLATION.get_or_init(|| Mutex::new(None));
        INTERRUPT_HANDLER
            .get_or_init(|| ctrlc::set_handler(cancel_active_job).map_err(|_| ()))
            .as_ref()
            .map_err(|_| ())?;
        let id = NEXT_CANCELLATION_ID.fetch_add(1, Ordering::Relaxed);
        *slot.lock().map_err(|_| ())? = Some((id, token));
        Ok(Self { id })
    }
}

fn cancel_active_job() {
    if let Some(slot) = ACTIVE_CANCELLATION.get()
        && let Ok(active) = slot.lock()
        && let Some((_, token)) = active.as_ref()
    {
        token.cancel();
    }
}

impl Drop for ActiveCancellation {
    fn drop(&mut self) {
        if let Some(slot) = ACTIVE_CANCELLATION.get()
            && let Ok(mut active) = slot.lock()
            && active.as_ref().is_some_and(|(id, _)| *id == self.id)
        {
            *active = None;
        }
    }
}

struct CliProgress<'a> {
    mode: ProgressArg,
    output: &'a mut dyn Write,
}

impl ProgressSink for CliProgress<'_> {
    fn stage_completed(&mut self, stage: JobStage) {
        match self.mode {
            ProgressArg::None => {}
            ProgressArg::Human => {
                let _ = writeln!(self.output, "completed {}", job_stage_name(stage));
            }
            ProgressArg::Json => {
                let _ = serde_json::to_writer(
                    &mut self.output,
                    &json!({"event": "stage_completed", "stage": stage}),
                );
                let _ = writeln!(self.output);
            }
        }
    }
}

fn emit_unavailable(
    json_output: bool,
    stdout: &mut dyn Write,
    stderr: &mut dyn Write,
    capability: &str,
) -> CommandExit {
    if json_output {
        let _ = write_json(
            stdout,
            &json!({"error": {"code": "unavailable", "capability": capability}}),
            stderr,
        );
    } else {
        human_error(stderr, "requested output codec is unavailable");
    }
    CommandExit::Unavailable
}

fn failure_exit(code: JobErrorCode) -> CommandExit {
    match code {
        JobErrorCode::Cancelled => CommandExit::Cancelled,
        JobErrorCode::RawBackend
        | JobErrorCode::EncoderBackendUnavailable
        | JobErrorCode::UnprovenPipelineBudget => CommandExit::Unavailable,
        JobErrorCode::InvalidOptions => CommandExit::Usage,
        JobErrorCode::Internal => CommandExit::Internal,
        _ => CommandExit::Failed,
    }
}

fn job_stage_name(stage: JobStage) -> &'static str {
    match stage {
        JobStage::Validate => "validate",
        JobStage::Decode => "decode",
        JobStage::ResolveSettings => "resolve_settings",
        JobStage::Develop => "develop",
        JobStage::SceneRender => "scene_render",
        JobStage::Encode => "encode",
        JobStage::Complete => "complete",
    }
}

fn error_name(code: JobErrorCode) -> &'static str {
    match code {
        JobErrorCode::InputIo => "input_io",
        JobErrorCode::UnsupportedFormat => "unsupported_format",
        JobErrorCode::CorruptInput => "corrupt_input",
        JobErrorCode::ColorManagement => "color_management",
        JobErrorCode::Metadata => "metadata",
        JobErrorCode::RawBackend => "raw_backend",
        JobErrorCode::ResourceLimit => "resource_limit",
        JobErrorCode::InvalidOptions => "invalid_options",
        JobErrorCode::Cancelled => "cancelled",
        JobErrorCode::Encode => "encode",
        JobErrorCode::OutputIo => "output_io",
        JobErrorCode::DestinationConflict => "destination_conflict",
        JobErrorCode::EncoderBackendUnavailable => "encoder_backend_unavailable",
        JobErrorCode::UnprovenPipelineBudget => "unproven_pipeline_budget",
        JobErrorCode::Internal => "internal",
    }
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
        serde_json::to_string_pretty(preset).map_err(omalux::develop::PresetError::Json)
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
    let raw_available = omalux::io::raw::trusted_dcraw_execution().is_ok();
    #[cfg(feature = "heic")]
    let heic = omalux::io::probe_heic_capability().ok();
    #[cfg(not(feature = "heic"))]
    let heic: Option<omalux::io::HeicCapability> = None;
    if json_output {
        write_json(
            stdout,
            &json!({
                "raw": {"available": raw_available, "backend": "libraw-dcraw-emu"},
                "heic": {
                    "available": heic.as_ref().is_some_and(|value| value.ten_bit),
                    "backend": "libheif-x265",
                    "libheif_version": heic.as_ref().map(|value| value.libheif_version.as_str()),
                    "encoder": heic.as_ref().map(|value| value.encoder.as_str()),
                    "eight_bit": heic.as_ref().is_some_and(|value| value.eight_bit),
                    "ten_bit": heic.as_ref().is_some_and(|value| value.ten_bit),
                }
            }),
            stderr,
        )
    } else if writeln!(
        stdout,
        "raw\tlibraw-dcraw-emu\t{}",
        if raw_available {
            "available"
        } else {
            "unavailable"
        }
    )
    .and_then(|()| {
        writeln!(
            stdout,
            "heic\tlibheif-x265\t{}",
            if heic.as_ref().is_some_and(|value| value.ten_bit) {
                "available"
            } else {
                "unavailable"
            }
        )
    })
    .is_ok()
    {
        CommandExit::Success
    } else {
        human_error(stderr, "probe output could not be written");
        CommandExit::Internal
    }
}

fn write_json<T: serde::Serialize + ?Sized>(
    stdout: &mut dyn Write,
    value: &T,
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
    let _ = writeln!(stderr, "omalux: {message}");
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
        ParameterUnit::Stops => "stops",
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
        let sibling = directory.path().join("omalux-gui");
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
        let core = directory.path().join("omalux");
        fs::write(&core, b"core").unwrap();
        let sibling = directory.path().join("omalux-gui");
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
        let core = directory.path().join("omalux");
        fs::write(&core, b"core").unwrap();
        let sibling = directory.path().join("omalux-gui");
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
        assert_eq!(output["presets"].as_array().unwrap().len(), 28);
        assert_eq!(output["presets"][0]["id"], "community-amber-grain");
        assert!(!String::from_utf8(stdout).unwrap().contains('/'));
    }
}
