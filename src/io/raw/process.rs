#[cfg(unix)]
use std::os::unix::{
    fs::PermissionsExt,
    process::{CommandExt, ExitStatusExt},
};
use std::{
    fs,
    io::{self, Read},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    time::{Duration, Instant},
};

use super::stage::StagedRaw;
use crate::io::{DecodeError, ResourceLimits, WhiteBalancePolicy};

const PRLIMIT: &str = "/usr/bin/prlimit";

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
        validate_executable(Path::new(PRLIMIT))?;
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
        validate_executable(Path::new(PRLIMIT))
    }
}

#[derive(Clone, Default, Debug)]
pub struct RawCancellation(std::sync::Arc<std::sync::atomic::AtomicBool>);
impl RawCancellation {
    pub fn cancel(&self) {
        self.0.store(true, std::sync::atomic::Ordering::Release)
    }
    pub(super) fn flag(&self) -> &std::sync::Arc<std::sync::atomic::AtomicBool> {
        &self.0
    }
    fn cancelled(&self) -> bool {
        self.0.load(std::sync::atomic::Ordering::Acquire)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RawCapability {
    Available {
        executable: PathBuf,
        prlimit: PathBuf,
    },
    Unavailable,
}
pub fn probe_dcraw_emu() -> RawCapability {
    match RawExecutionOptions::new(Path::new("dcraw_emu")) {
        Ok(options) => RawCapability::Available {
            executable: options.executable,
            prlimit: PathBuf::from(PRLIMIT),
        },
        Err(_) => RawCapability::Unavailable,
    }
}

pub(super) fn run_dcraw(
    staged: &StagedRaw,
    white_balance: WhiteBalancePolicy,
    limits: &ResourceLimits,
    execution: &RawExecutionOptions,
    cancellation: &RawCancellation,
) -> Result<(), DecodeError> {
    execution.validate()?;
    if cancellation.cancelled() {
        return Err(DecodeError::Cancelled);
    }
    let args = dcraw_args(&staged.input_path(), &staged.output_path(), white_balance);
    let file_limit = limits
        .max_decoded_bytes
        .checked_add(64 * 1024)
        .ok_or(DecodeError::Limit(
            crate::io::LimitError::ArithmeticOverflow,
        ))?;
    let cpu_seconds = execution
        .timeout
        .as_secs()
        .saturating_add(u64::from(execution.timeout.subsec_nanos() > 0))
        .max(1);
    let mut command = Command::new(PRLIMIT);
    command
        .arg(format!("--as={}", limits.max_working_bytes))
        .arg(format!("--data={}", limits.max_working_bytes))
        .arg(format!("--fsize={file_limit}"))
        .arg(format!("--cpu={cpu_seconds}"))
        .arg("--")
        .arg(&execution.executable)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped());
    #[cfg(unix)]
    command.process_group(0);
    let mut child = command.spawn().map_err(|e| {
        if e.kind() == io::ErrorKind::NotFound {
            DecodeError::RawBackendUnavailable
        } else {
            DecodeError::Input(e)
        }
    })?;
    let pgid = child.id();
    let leader = match rustix::process::Pid::from_raw(pgid as i32) {
        Some(leader) => leader,
        None => {
            kill_group_and_wait(pgid, &mut child);
            return Err(DecodeError::RawBackendCaptureIo(io::Error::other(
                "invalid decoder process id",
            )));
        }
    };
    let mut stderr = match child.stderr.take() {
        Some(stderr) => stderr,
        None => {
            kill_group_and_wait(pgid, &mut child);
            return Err(DecodeError::RawBackendCaptureIo(io::Error::other(
                "stderr capture pipe unavailable",
            )));
        }
    };
    let flags = match rustix::fs::fcntl_getfl(&stderr) {
        Ok(flags) => flags,
        Err(error) => {
            kill_group_and_wait(pgid, &mut child);
            return Err(DecodeError::RawBackendCaptureIo(std_error(error)));
        }
    };
    if let Err(error) = rustix::fs::fcntl_setfl(&stderr, flags | rustix::fs::OFlags::NONBLOCK) {
        kill_group_and_wait(pgid, &mut child);
        return Err(DecodeError::RawBackendCaptureIo(std_error(error)));
    }
    let started = Instant::now();
    let mut observed_exit = None;
    let mut eof = false;
    let mut stderr_bytes = 0_u64;
    let mut buffer = [0_u8; 8192];
    loop {
        drain_stderr(
            &mut stderr,
            &mut buffer,
            &mut stderr_bytes,
            execution.max_stderr_bytes,
            &mut eof,
        )
        .inspect_err(|_| {
            kill_group_and_wait(pgid, &mut child);
        })?;
        if observed_exit.is_none() {
            observed_exit = match observe_exit_without_reaping(leader) {
                Ok(status) => status,
                Err(error) => {
                    kill_group_and_wait(pgid, &mut child);
                    return Err(error);
                }
            };
        }
        if observed_exit.is_some() && eof {
            break;
        }
        if cancellation.cancelled() {
            kill_group_and_wait(pgid, &mut child);
            drain_after_kill(&mut stderr)?;
            return Err(DecodeError::Cancelled);
        }
        if started.elapsed() >= execution.timeout {
            kill_group_and_wait(pgid, &mut child);
            drain_after_kill(&mut stderr)?;
            return Err(DecodeError::RawBackendTimedOut);
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    let observed_exit = observed_exit.expect("leader exit observed when monitor completes");
    if !observed_exit.success() {
        kill_group(pgid);
        let status = child.wait().map_err(DecodeError::RawBackendCaptureIo)?;
        return map_exit_status(status);
    }
    if let Err(error) = terminate_success_survivors_while_leader_is_pinned(leader) {
        kill_group(pgid);
        let _ = child.wait();
        return Err(error);
    }
    let status = child.wait().map_err(DecodeError::RawBackendCaptureIo)?;
    map_exit_status(status)
}

#[derive(Clone, Copy)]
struct ObservedExit {
    code: Option<i32>,
}
impl ObservedExit {
    fn success(self) -> bool {
        self.code == Some(0)
    }
}

fn observe_exit_without_reaping(
    leader: rustix::process::Pid,
) -> Result<Option<ObservedExit>, DecodeError> {
    rustix::process::waitid(
        rustix::process::WaitId::Pid(leader),
        rustix::process::WaitIdOptions::EXITED
            | rustix::process::WaitIdOptions::NOHANG
            | rustix::process::WaitIdOptions::NOWAIT,
    )
    .map(|status| {
        status.map(|status| ObservedExit {
            code: status.exit_status(),
        })
    })
    .map_err(|error| DecodeError::RawBackendCaptureIo(std_error(error)))
}

fn terminate_success_survivors_while_leader_is_pinned(
    leader: rustix::process::Pid,
) -> Result<(), DecodeError> {
    // WNOWAIT must still observe the leader here. Its unreaped zombie pins the
    // numeric PID/PGID, so the following group signal cannot target a reused ID.
    if observe_exit_without_reaping(leader)?.is_none() {
        return Err(DecodeError::RawBackendCaptureIo(io::Error::other(
            "decoder leader was reaped before process-group cleanup",
        )));
    }
    #[cfg(test)]
    PINNED_LEADER_CHECKS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let group = leader;
    match rustix::process::test_kill_process_group(group) {
        Err(rustix::io::Errno::SRCH) => {
            return Err(DecodeError::RawBackendCaptureIo(io::Error::other(
                "pinned decoder process group unexpectedly disappeared",
            )));
        }
        Err(error) => return Err(DecodeError::RawBackendCaptureIo(std_error(error))),
        Ok(()) => {}
    }
    rustix::process::kill_process_group(group, rustix::process::Signal::KILL)
        .map_err(|error| DecodeError::RawBackendCaptureIo(std_error(error)))?;
    std::thread::sleep(Duration::from_millis(10));
    if observe_exit_without_reaping(leader)?.is_none() {
        return Err(DecodeError::RawBackendCaptureIo(io::Error::other(
            "decoder leader lost before exact reap",
        )));
    }
    #[cfg(test)]
    PINNED_LEADER_CHECKS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    Ok(())
}

#[cfg(test)]
static PINNED_LEADER_CHECKS: std::sync::atomic::AtomicUsize =
    std::sync::atomic::AtomicUsize::new(0);

fn map_exit_status(status: std::process::ExitStatus) -> Result<(), DecodeError> {
    if status.success() {
        return Ok(());
    }
    let xfsz = rustix::process::Signal::XFSZ.as_raw();
    let xcpu = rustix::process::Signal::XCPU.as_raw();
    if status.signal() == Some(xfsz) || status.code() == Some(128 + xfsz) {
        return Err(DecodeError::RawBackendOutputLimit);
    }
    if status.signal() == Some(xcpu) || status.code() == Some(128 + xcpu) {
        return Err(DecodeError::RawBackendTimedOut);
    }
    Err(DecodeError::RawBackendFailed {
        status: status.code(),
    })
}

fn drain_stderr(
    stderr: &mut impl Read,
    buffer: &mut [u8],
    total: &mut u64,
    limit: u64,
    eof: &mut bool,
) -> Result<(), DecodeError> {
    loop {
        match stderr.read(buffer) {
            Ok(0) => {
                *eof = true;
                return Ok(());
            }
            Ok(count) => {
                *total = total
                    .checked_add(count as u64)
                    .ok_or(DecodeError::RawBackendOutputLimit)?;
                if *total > limit {
                    return Err(DecodeError::RawBackendOutputLimit);
                }
            }
            Err(e) if e.kind() == io::ErrorKind::WouldBlock => return Ok(()),
            Err(e) => return Err(DecodeError::RawBackendCaptureIo(e)),
        }
    }
}
fn drain_after_kill(stderr: &mut impl Read) -> Result<(), DecodeError> {
    let deadline = Instant::now() + Duration::from_secs(1);
    let mut buffer = [0_u8; 8192];
    loop {
        match stderr.read(&mut buffer) {
            Ok(0) => return Ok(()),
            Ok(_) => {}
            Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                if Instant::now() >= deadline {
                    return Err(DecodeError::RawBackendCaptureIo(io::Error::new(
                        io::ErrorKind::TimedOut,
                        "stderr pipe did not close after process-group kill",
                    )));
                }
                std::thread::sleep(Duration::from_millis(5));
            }
            Err(e) => return Err(DecodeError::RawBackendCaptureIo(e)),
        }
    }
}

fn dcraw_args(
    input: &str,
    output: &str,
    white_balance: WhiteBalancePolicy,
) -> Vec<std::ffi::OsString> {
    let mut args = Vec::new();
    match white_balance {
        WhiteBalancePolicy::CameraThenDaylight => args.push("-w".into()),
        WhiteBalancePolicy::Daylight => {}
        WhiteBalancePolicy::Explicit(v) => {
            args.push("-r".into());
            for x in v {
                args.push(x.to_string().into());
            }
        }
    }
    args.extend(["+M", "-H", "0", "-q", "3", "-4", "-o", "8", "-Z"].map(Into::into));
    args.push(output.into());
    args.push(input.into());
    args
}
fn kill_group_and_wait(pgid: u32, child: &mut std::process::Child) {
    kill_group(pgid);
    #[cfg(not(target_os = "linux"))]
    let _ = child.kill();
    let _ = child.wait();
}
fn kill_group(pgid: u32) {
    #[cfg(target_os = "linux")]
    if let Some(pid) = rustix::process::Pid::from_raw(pgid as i32) {
        let _ = rustix::process::kill_process_group(pid, rustix::process::Signal::KILL);
    }
}
fn resolve_executable(requested: &Path) -> Result<PathBuf, DecodeError> {
    let candidate = if requested.components().count() > 1 || requested.is_absolute() {
        requested.to_owned()
    } else {
        std::env::var_os("PATH")
            .and_then(|p| {
                std::env::split_paths(&p)
                    .map(|d| d.join(requested))
                    .find(|p| p.is_file())
            })
            .ok_or(DecodeError::RawBackendUnavailable)?
    };
    let resolved = fs::canonicalize(candidate).map_err(|e| {
        if e.kind() == io::ErrorKind::NotFound {
            DecodeError::RawBackendUnavailable
        } else {
            DecodeError::Input(e)
        }
    })?;
    validate_executable(&resolved)?;
    Ok(resolved)
}
fn validate_executable(path: &Path) -> Result<(), DecodeError> {
    let metadata = fs::metadata(path).map_err(|_| DecodeError::RawBackendUnavailable)?;
    #[cfg(unix)]
    if !metadata.is_file() || metadata.permissions().mode() & 0o111 == 0 {
        return Err(DecodeError::RawBackendUnavailable);
    }
    if !path.is_absolute() {
        return Err(DecodeError::RawBackendUnavailable);
    }
    Ok(())
}
fn std_error(error: rustix::io::Errno) -> io::Error {
    io::Error::from_raw_os_error(error.raw_os_error())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;
    use tempfile::tempdir;
    fn script(directory: &Path, body: &str) -> PathBuf {
        let p = directory.join("fake-dcraw");
        fs::write(&p, format!("#!/bin/sh\n{body}\n")).unwrap();
        fs::set_permissions(&p, fs::Permissions::from_mode(0o700)).unwrap();
        p
    }
    fn staged(d: &tempfile::TempDir) -> StagedRaw {
        let source = d.path().join("input.nef");
        fs::write(&source, b"raw").unwrap();
        super::super::stage::stage_source(
            &source,
            d.path(),
            &ResourceLimits::default(),
            RawCancellation::default().flag(),
        )
        .unwrap()
    }
    fn assert_not_running(pid: &str) {
        let stat = fs::read_to_string(format!("/proc/{}/stat", pid.trim()));
        if let Ok(stat) = stat {
            let state = stat.rsplit_once(") ").unwrap().1.as_bytes()[0];
            assert_eq!(
                state, b'Z',
                "descendant survived process-group kill: {stat}"
            );
        }
    }
    #[test]
    fn argv_is_fullres_and_file_output() {
        let a = dcraw_args(
            "/proc/self/fd/9/in.nef",
            "/proc/self/fd/9/out.ppm",
            WhiteBalancePolicy::CameraThenDaylight,
        );
        let s = a.iter().map(|x| x.to_string_lossy()).collect::<Vec<_>>();
        assert_eq!(
            &s[..11],
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
                "/proc/self/fd/9/out.ppm"
            ]
        );
        assert!(
            !s.iter()
                .any(|x| x == "-h" || x == "-t" || x == "-j" || x == "-")
        );
    }
    #[test]
    fn descendants_holding_pipe_force_timeout_and_group_kill() {
        let d = tempdir().unwrap();
        let staged = staged(&d);
        let pidfile = d.path().join("timeout-child-pid");
        let exe = script(
            d.path(),
            &format!("sleep 5 & echo $! > '{}'; exit 0", pidfile.display()),
        );
        let mut o = RawExecutionOptions::new(exe).unwrap();
        o.timeout = Duration::from_millis(40);
        let start = Instant::now();
        assert!(matches!(
            run_dcraw(
                &staged,
                WhiteBalancePolicy::Daylight,
                &ResourceLimits::default(),
                &o,
                &RawCancellation::default()
            ),
            Err(DecodeError::RawBackendTimedOut)
        ));
        assert!(start.elapsed() < Duration::from_secs(1));
        let pid = fs::read_to_string(pidfile).unwrap();
        assert_not_running(&pid);
    }
    #[test]
    fn monitor_accepts_descendant_write_after_leader_exit_then_eof() {
        let d = tempdir().unwrap();
        let staged = staged(&d);
        let exe = script(d.path(), "(sleep 0.03; printf late >&2) & exit 0");
        let o = RawExecutionOptions::new(exe).unwrap();
        let start = Instant::now();
        run_dcraw(
            &staged,
            WhiteBalancePolicy::Daylight,
            &ResourceLimits::default(),
            &o,
            &RawCancellation::default(),
        )
        .unwrap();
        assert!(start.elapsed() >= Duration::from_millis(20));
    }
    #[test]
    fn apparent_success_kills_descendant_that_closed_capture_pipe() {
        let d = tempdir().unwrap();
        let staged = staged(&d);
        let pidfile = d.path().join("detached-pid");
        let exe = script(
            d.path(),
            &format!(
                "sleep 5 2>/dev/null & echo $! > '{}'; exit 0",
                pidfile.display()
            ),
        );
        let checks_before = PINNED_LEADER_CHECKS.load(std::sync::atomic::Ordering::Relaxed);
        run_dcraw(
            &staged,
            WhiteBalancePolicy::Daylight,
            &ResourceLimits::default(),
            &RawExecutionOptions::new(exe).unwrap(),
            &RawCancellation::default(),
        )
        .unwrap();
        assert!(
            PINNED_LEADER_CHECKS.load(std::sync::atomic::Ordering::Relaxed) >= checks_before + 2
        );
        assert_not_running(&fs::read_to_string(pidfile).unwrap());
    }
    #[test]
    fn cancel_kills_descendant_without_survivor() {
        let d = tempdir().unwrap();
        let staged = staged(&d);
        let pidfile = d.path().join("pid");
        let exe = script(
            d.path(),
            &format!("sleep 5 & echo $! > '{}'; sleep 5", pidfile.display()),
        );
        let o = RawExecutionOptions::new(exe).unwrap();
        let c = RawCancellation::default();
        let trigger = c.clone();
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(30));
            trigger.cancel();
        });
        assert!(matches!(
            run_dcraw(
                &staged,
                WhiteBalancePolicy::Daylight,
                &ResourceLimits::default(),
                &o,
                &c
            ),
            Err(DecodeError::Cancelled)
        ));
        let pid = fs::read_to_string(pidfile).unwrap();
        assert_not_running(&pid);
    }
    #[test]
    fn stderr_limit_is_distinct() {
        let d = tempdir().unwrap();
        let staged = staged(&d);
        let exe = script(d.path(), "while :; do printf x >&2; done");
        let mut o = RawExecutionOptions::new(exe).unwrap();
        o.max_stderr_bytes = 32;
        assert!(matches!(
            run_dcraw(
                &staged,
                WhiteBalancePolicy::Daylight,
                &ResourceLimits::default(),
                &o,
                &RawCancellation::default()
            ),
            Err(DecodeError::RawBackendOutputLimit)
        ));
    }
    #[test]
    fn nonzero_exit_is_stable_backend_failure() {
        let d = tempdir().unwrap();
        let staged = staged(&d);
        let exe = script(d.path(), "printf diagnostic >&2; exit 23");
        assert!(matches!(
            run_dcraw(
                &staged,
                WhiteBalancePolicy::Daylight,
                &ResourceLimits::default(),
                &RawExecutionOptions::new(exe).unwrap(),
                &RawCancellation::default()
            ),
            Err(DecodeError::RawBackendFailed { status: Some(23) })
        ));
    }
    #[test]
    fn file_size_rlimit_is_a_distinct_output_limit() {
        let d = tempdir().unwrap();
        let staged = staged(&d);
        let exe = script(
            d.path(),
            "out=''\nwhile [ \"$#\" -gt 0 ]; do\n  if [ \"$1\" = '-Z' ]; then shift; out=$1; fi\n  shift\ndone\nhead -c 131072 /dev/zero > \"$out\"",
        );
        let limits = ResourceLimits {
            max_decoded_bytes: 8,
            ..ResourceLimits::default()
        };
        assert!(matches!(
            run_dcraw(
                &staged,
                WhiteBalancePolicy::Daylight,
                &limits,
                &RawExecutionOptions::new(exe).unwrap(),
                &RawCancellation::default()
            ),
            Err(DecodeError::RawBackendOutputLimit)
        ));
    }
    #[test]
    fn inherited_directory_fd_survives_parent_swap() {
        let d = tempdir().unwrap();
        let stage_path = d.path().join("stage");
        fs::create_dir(&stage_path).unwrap();
        let source = d.path().join("source.nef");
        fs::write(&source, b"raw through inherited descriptor").unwrap();
        let staged = super::super::stage::stage_source(
            &source,
            &stage_path,
            &ResourceLimits::default(),
            RawCancellation::default().flag(),
        )
        .unwrap();
        fs::rename(&stage_path, d.path().join("moved-stage")).unwrap();
        fs::create_dir(&stage_path).unwrap();
        let exe = script(
            d.path(),
            "out=''\nlast=''\nwhile [ \"$#\" -gt 0 ]; do\n  last=$1\n  if [ \"$1\" = '-Z' ]; then shift; out=$1; fi\n  shift\ndone\ncp \"$last\" \"$out\"",
        );
        run_dcraw(
            &staged,
            WhiteBalancePolicy::Daylight,
            &ResourceLimits::default(),
            &RawExecutionOptions::new(exe).unwrap(),
            &RawCancellation::default(),
        )
        .unwrap();
        assert_eq!(
            fs::read(staged.output_path()).unwrap(),
            b"raw through inherited descriptor"
        );
        assert_eq!(fs::read_dir(stage_path).unwrap().count(), 0);
    }
    #[test]
    fn capability_is_explicit() {
        match probe_dcraw_emu() {
            RawCapability::Available {
                executable,
                prlimit,
            } => {
                assert!(executable.is_absolute());
                assert_eq!(prlimit, Path::new(PRLIMIT));
            }
            RawCapability::Unavailable => {}
        }
    }
}
