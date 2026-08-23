#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
#[cfg(unix)]
use std::os::unix::process::CommandExt;
use std::{
    fs,
    io::{self, Read},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::{Duration, Instant},
};

use crate::io::{DecodeError, ResourceLimits, WhiteBalancePolicy};

#[derive(Clone, Debug)]
pub struct RawExecutionOptions {
    executable: PathBuf,
    pub timeout: Duration,
    pub max_stderr_bytes: u64,
    pub staging_directory: PathBuf,
}
impl RawExecutionOptions {
    pub fn new(executable: impl AsRef<Path>) -> Result<Self, DecodeError> {
        let executable = resolve_executable(executable.as_ref())?;
        Ok(Self {
            executable,
            timeout: Duration::from_secs(120),
            max_stderr_bytes: 1 << 20,
            staging_directory: std::env::temp_dir(),
        })
    }
    pub fn executable(&self) -> &Path {
        &self.executable
    }
    pub fn validate(&self) -> Result<(), DecodeError> {
        if self.timeout.is_zero() || self.max_stderr_bytes == 0 {
            return Err(DecodeError::InvalidOptions);
        }
        Ok(())
    }
}

#[derive(Clone, Default, Debug)]
pub struct RawCancellation(Arc<AtomicBool>);
impl RawCancellation {
    pub fn cancel(&self) {
        self.0.store(true, Ordering::Release)
    }
    pub(super) fn flag(&self) -> &Arc<AtomicBool> {
        &self.0
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RawCapability {
    Available { executable: PathBuf },
    Unavailable,
}
pub fn probe_dcraw_emu() -> RawCapability {
    match RawExecutionOptions::new(Path::new("dcraw_emu")) {
        Ok(options) => RawCapability::Available {
            executable: options.executable,
        },
        Err(_) => RawCapability::Unavailable,
    }
}

pub(super) fn run_dcraw(
    staged: &Path,
    white_balance: WhiteBalancePolicy,
    limits: &ResourceLimits,
    execution: &RawExecutionOptions,
    cancellation: &RawCancellation,
) -> Result<Vec<u8>, DecodeError> {
    execution.validate()?;
    if cancellation.0.load(Ordering::Acquire) {
        return Err(DecodeError::Cancelled);
    }
    let args = dcraw_args(staged, white_balance);
    let mut command = Command::new(&execution.executable);
    command
        .args(&args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    #[cfg(unix)]
    command.process_group(0);
    let mut child = command.spawn().map_err(|error| {
        if error.kind() == io::ErrorKind::NotFound {
            DecodeError::RawBackendUnavailable
        } else {
            DecodeError::Input(error)
        }
    })?;
    let overflow = Arc::new(AtomicBool::new(false));
    let stdout = child
        .stdout
        .take()
        .ok_or(DecodeError::RawBackendFailed { status: None })?;
    let stderr = child
        .stderr
        .take()
        .ok_or(DecodeError::RawBackendFailed { status: None })?;
    let stdout_limit =
        limits
            .max_decoded_bytes
            .checked_add(64 * 1024)
            .ok_or(DecodeError::Limit(
                crate::io::LimitError::ArithmeticOverflow,
            ))?;
    let stdout_thread = spawn_capture(stdout, stdout_limit, overflow.clone());
    let stderr_thread = spawn_capture(stderr, execution.max_stderr_bytes, overflow.clone());
    let started = Instant::now();
    let status = loop {
        if cancellation.0.load(Ordering::Acquire) {
            kill_and_wait(&mut child);
            join_discard(stdout_thread, stderr_thread);
            return Err(DecodeError::Cancelled);
        }
        if overflow.load(Ordering::Acquire) {
            kill_and_wait(&mut child);
            join_discard(stdout_thread, stderr_thread);
            return Err(DecodeError::RawBackendOutputLimit);
        }
        if started.elapsed() >= execution.timeout {
            kill_and_wait(&mut child);
            join_discard(stdout_thread, stderr_thread);
            return Err(DecodeError::RawBackendTimedOut);
        }
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) => thread::sleep(Duration::from_millis(5)),
            Err(error) => {
                kill_and_wait(&mut child);
                join_discard(stdout_thread, stderr_thread);
                return Err(DecodeError::Input(error));
            }
        }
    };
    let stdout = stdout_thread
        .join()
        .map_err(|_| DecodeError::RawBackendFailed {
            status: status.code(),
        })?
        .map_err(|_| DecodeError::RawBackendOutputLimit)?;
    let _stderr = stderr_thread
        .join()
        .map_err(|_| DecodeError::RawBackendFailed {
            status: status.code(),
        })?
        .map_err(|_| DecodeError::RawBackendOutputLimit)?;
    if !status.success() {
        return Err(DecodeError::RawBackendFailed {
            status: status.code(),
        });
    }
    Ok(stdout)
}

fn dcraw_args(staged: &Path, white_balance: WhiteBalancePolicy) -> Vec<std::ffi::OsString> {
    let mut args = Vec::new();
    match white_balance {
        WhiteBalancePolicy::CameraThenDaylight => args.push("-w".into()),
        WhiteBalancePolicy::Daylight => {}
        WhiteBalancePolicy::Explicit(values) => {
            args.push("-r".into());
            for value in values {
                args.push(value.to_string().into());
            }
        }
    }
    args.extend(["+M", "-H", "0", "-q", "3", "-4", "-o", "8", "-Z", "-"].map(Into::into));
    args.push(staged.as_os_str().to_owned());
    args
}

fn spawn_capture(
    mut reader: impl Read + Send + 'static,
    limit: u64,
    overflow: Arc<AtomicBool>,
) -> thread::JoinHandle<io::Result<Vec<u8>>> {
    thread::spawn(move || {
        let capacity = usize::try_from(limit.min(1 << 20)).unwrap_or(1 << 20);
        let mut output = Vec::with_capacity(capacity);
        let mut buffer = [0_u8; 8192];
        loop {
            let count = reader.read(&mut buffer)?;
            if count == 0 {
                break;
            }
            let next = u64::try_from(output.len())
                .unwrap_or(u64::MAX)
                .saturating_add(count as u64);
            if next > limit {
                overflow.store(true, Ordering::Release);
                return Err(io::Error::new(
                    io::ErrorKind::FileTooLarge,
                    "backend output limit",
                ));
            }
            output.extend_from_slice(&buffer[..count]);
        }
        Ok(output)
    })
}
fn kill_and_wait(child: &mut std::process::Child) {
    #[cfg(target_os = "linux")]
    if let Some(pid) = rustix::process::Pid::from_raw(child.id() as i32) {
        let _ = rustix::process::kill_process_group(pid, rustix::process::Signal::KILL);
    }
    #[cfg(not(target_os = "linux"))]
    let _ = child.kill();
    let _ = child.wait();
}
fn join_discard(
    a: thread::JoinHandle<io::Result<Vec<u8>>>,
    b: thread::JoinHandle<io::Result<Vec<u8>>>,
) {
    let _ = a.join();
    let _ = b.join();
}

fn resolve_executable(requested: &Path) -> Result<PathBuf, DecodeError> {
    let candidate = if requested.components().count() > 1 || requested.is_absolute() {
        requested.to_owned()
    } else {
        std::env::var_os("PATH")
            .and_then(|paths| {
                std::env::split_paths(&paths)
                    .map(|dir| dir.join(requested))
                    .find(|path| path.is_file())
            })
            .ok_or(DecodeError::RawBackendUnavailable)?
    };
    let resolved = fs::canonicalize(candidate).map_err(|error| {
        if error.kind() == io::ErrorKind::NotFound {
            DecodeError::RawBackendUnavailable
        } else {
            DecodeError::Input(error)
        }
    })?;
    let metadata = fs::metadata(&resolved).map_err(DecodeError::Input)?;
    #[cfg(unix)]
    if !metadata.is_file() || metadata.permissions().mode() & 0o111 == 0 {
        return Err(DecodeError::RawBackendUnavailable);
    }
    if !resolved.is_absolute() {
        return Err(DecodeError::RawBackendUnavailable);
    }
    Ok(resolved)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;
    #[cfg(unix)]
    fn script(directory: &Path, body: &str) -> PathBuf {
        let path = directory.join("fake-dcraw");
        fs::write(&path, format!("#!/bin/sh\n{body}\n")).unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o700)).unwrap();
        path
    }
    #[test]
    fn argv_is_exact_full_resolution_and_no_shell() {
        let args = dcraw_args(
            Path::new("literal;touch BAD.nef"),
            WhiteBalancePolicy::CameraThenDaylight,
        );
        let strings = args.iter().map(|s| s.to_string_lossy()).collect::<Vec<_>>();
        assert_eq!(
            &strings[..12],
            [
                "-w",
                "+M",
                "-H",
                "0",
                "-q",
                "3",
                "-4",
                "-o",
                "8",
                "-Z",
                "-",
                "literal;touch BAD.nef"
            ]
        );
        assert!(!strings.iter().any(|s| s == "-h" || s == "-t" || s == "-j"));
    }
    #[cfg(unix)]
    #[test]
    fn timeout_kills_and_waits() {
        let d = tempdir().unwrap();
        let executable = script(d.path(), "sleep 5");
        let mut options = RawExecutionOptions::new(executable).unwrap();
        options.timeout = Duration::from_millis(30);
        let result = run_dcraw(
            Path::new("input.nef"),
            WhiteBalancePolicy::CameraThenDaylight,
            &ResourceLimits::default(),
            &options,
            &RawCancellation::default(),
        );
        assert!(matches!(result, Err(DecodeError::RawBackendTimedOut)));
    }
    #[cfg(unix)]
    #[test]
    fn stdout_and_stderr_are_drained_concurrently() {
        let d = tempdir().unwrap();
        let executable = script(
            d.path(),
            "i=0; while [ $i -lt 5000 ]; do printf x >&2; i=$((i+1)); done; printf 'P6\\n1 1\\n65535\\n\\377\\377\\000\\000\\000\\000'",
        );
        let mut options = RawExecutionOptions::new(executable).unwrap();
        options.max_stderr_bytes = 10_000;
        let bytes = run_dcraw(
            Path::new("input.nef"),
            WhiteBalancePolicy::CameraThenDaylight,
            &ResourceLimits::default(),
            &options,
            &RawCancellation::default(),
        )
        .unwrap();
        assert!(bytes.starts_with(b"P6"));
    }
    #[cfg(unix)]
    #[test]
    fn stderr_limit_kills_backend() {
        let d = tempdir().unwrap();
        let executable = script(d.path(), "while :; do printf x >&2; done");
        let mut options = RawExecutionOptions::new(executable).unwrap();
        options.max_stderr_bytes = 32;
        assert!(matches!(
            run_dcraw(
                Path::new("input.nef"),
                WhiteBalancePolicy::CameraThenDaylight,
                &ResourceLimits::default(),
                &options,
                &RawCancellation::default()
            ),
            Err(DecodeError::RawBackendOutputLimit)
        ));
    }
    #[cfg(unix)]
    #[test]
    fn stdout_limit_kills_backend() {
        let d = tempdir().unwrap();
        let executable = script(d.path(), "while :; do printf 1234567890; done");
        let options = RawExecutionOptions::new(executable).unwrap();
        let limits = ResourceLimits {
            max_decoded_bytes: 8,
            ..Default::default()
        };
        let result = run_dcraw(
            Path::new("input.nef"),
            WhiteBalancePolicy::Daylight,
            &limits,
            &options,
            &RawCancellation::default(),
        );
        assert!(
            matches!(result, Err(DecodeError::RawBackendOutputLimit)),
            "{result:?}"
        );
    }
    #[cfg(unix)]
    #[test]
    fn cancellation_kills_running_process_group() {
        let d = tempdir().unwrap();
        let executable = script(d.path(), "sleep 5");
        let options = RawExecutionOptions::new(executable).unwrap();
        let cancellation = RawCancellation::default();
        let trigger = cancellation.clone();
        let thread = std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(20));
            trigger.cancel();
        });
        let started = Instant::now();
        let result = run_dcraw(
            Path::new("input.nef"),
            WhiteBalancePolicy::CameraThenDaylight,
            &ResourceLimits::default(),
            &options,
            &cancellation,
        );
        thread.join().unwrap();
        assert!(matches!(result, Err(DecodeError::Cancelled)));
        assert!(started.elapsed() < Duration::from_secs(1));
    }
    #[cfg(unix)]
    #[test]
    fn nonzero_exit_is_stable_backend_failure() {
        let d = tempdir().unwrap();
        let executable = script(d.path(), "printf diagnostic >&2; exit 23");
        let options = RawExecutionOptions::new(executable).unwrap();
        assert!(matches!(
            run_dcraw(
                Path::new("input.nef"),
                WhiteBalancePolicy::Daylight,
                &ResourceLimits::default(),
                &options,
                &RawCancellation::default()
            ),
            Err(DecodeError::RawBackendFailed { status: Some(23) })
        ));
    }
    #[test]
    fn pre_cancel_is_explicit() {
        let cancel = RawCancellation::default();
        cancel.cancel();
        let options = RawExecutionOptions {
            executable: PathBuf::from("/missing"),
            timeout: Duration::from_secs(1),
            max_stderr_bytes: 1,
            staging_directory: PathBuf::from("."),
        };
        assert!(matches!(
            run_dcraw(
                Path::new("x"),
                WhiteBalancePolicy::Daylight,
                &ResourceLimits::default(),
                &options,
                &cancel
            ),
            Err(DecodeError::Cancelled)
        ));
    }
}
