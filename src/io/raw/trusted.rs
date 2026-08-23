//! Fixed-path, fail-closed discovery of the production LibRaw backend.

use std::{
    fs,
    io::{self, Read},
    os::unix::{
        fs::{MetadataExt, PermissionsExt},
        process::CommandExt,
    },
    path::{Path, PathBuf},
    process::{Command, Stdio},
    time::{Duration, Instant},
};

use crate::io::DecodeError;

use super::RawExecutionOptions;

const TRUSTED_CANDIDATES: [&str; 2] = ["/usr/bin/dcraw_emu", "/usr/local/bin/dcraw_emu"];
const PROBE_SENTINEL: &str = "/proc/self/fd/2147483647";
const PROBE_CAPTURE_BYTES: usize = 8 * 1024;

/// Resolves the first functional backend from the two fixed package paths.
/// `PATH` is never consulted, and the exact same result feeds both capability
/// reporting and production RAW decoding.
pub fn trusted_dcraw_execution() -> Result<RawExecutionOptions, DecodeError> {
    let candidates = TRUSTED_CANDIDATES.map(PathBuf::from);
    resolve_trusted(&candidates, trusted_metadata)
        .ok_or(DecodeError::RawBackendUnavailable)
        .and_then(RawExecutionOptions::new)
}

fn resolve_trusted(candidates: &[PathBuf], trusted: impl Fn(&Path) -> bool) -> Option<PathBuf> {
    candidates
        .iter()
        .find(|path| trusted(path) && functional_handshake(path))
        .cloned()
}

fn trusted_metadata(path: &Path) -> bool {
    let Ok(metadata) = fs::symlink_metadata(path) else {
        return false;
    };
    path.is_absolute()
        && metadata.file_type().is_file()
        && metadata.uid() == 0
        && metadata.permissions().mode() & 0o111 != 0
        && metadata.permissions().mode() & 0o022 == 0
}

fn functional_handshake(executable: &Path) -> bool {
    let mut command = Command::new(executable);
    command
        .arg("-v")
        .arg(PROBE_SENTINEL)
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
        kill_group(process_group);
        let _ = child.wait();
        return false;
    };
    let (Some(mut stdout), Some(mut stderr)) = (child.stdout.take(), child.stderr.take()) else {
        kill_group(process_group);
        let _ = child.wait();
        return false;
    };
    if !set_nonblocking(&stdout) || !set_nonblocking(&stderr) {
        kill_group(process_group);
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
        if !drain_stream(&mut stdout, &mut stdout_bytes, &mut stdout_eof)
            || !drain_stream(&mut stderr, &mut stderr_bytes, &mut stderr_eof)
        {
            break false;
        }
        if observed_exit.is_none() {
            match observe_exit(leader) {
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
        kill_group(process_group);
        let _ = child.wait();
        return false;
    }
    let Some(exit_code) = observed_exit else {
        kill_group(process_group);
        let _ = child.wait();
        return false;
    };
    if observe_exit(leader).ok().flatten().is_none() {
        kill_group(process_group);
        let _ = child.wait();
        return false;
    }
    kill_group(process_group);
    std::thread::sleep(Duration::from_millis(10));
    if observe_exit(leader).ok().flatten().is_none() {
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
        && output.contains(&format!("Processing file {PROBE_SENTINEL}"))
        && output.contains("Cannot open")
}

fn set_nonblocking(file: &impl std::os::fd::AsFd) -> bool {
    let Ok(flags) = rustix::fs::fcntl_getfl(file) else {
        return false;
    };
    rustix::fs::fcntl_setfl(file, flags | rustix::fs::OFlags::NONBLOCK).is_ok()
}

fn drain_stream(stream: &mut impl Read, retained: &mut Vec<u8>, eof: &mut bool) -> bool {
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
                if new_len > PROBE_CAPTURE_BYTES {
                    return false;
                }
                retained.extend_from_slice(&chunk[..count]);
            }
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => return true,
            Err(_) => return false,
        }
    }
}

fn observe_exit(leader: rustix::process::Pid) -> Result<Option<i32>, ()> {
    rustix::process::waitid(
        rustix::process::WaitId::Pid(leader),
        rustix::process::WaitIdOptions::EXITED
            | rustix::process::WaitIdOptions::NOHANG
            | rustix::process::WaitIdOptions::NOWAIT,
    )
    .map(|status| status.and_then(|status| status.exit_status()))
    .map_err(|_| ())
}

fn kill_group(process_group: u32) {
    if let Some(pid) = rustix::process::Pid::from_raw(process_group as i32) {
        let _ = rustix::process::kill_process_group(pid, rustix::process::Signal::KILL);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;
    use tempfile::tempdir;

    fn valid_script(path: &Path) {
        fs::write(
            path,
            format!(
                "#!/bin/sh\nprintf 'Using 4 threads\\nProcessing file {PROBE_SENTINEL}\\nCannot open {PROBE_SENTINEL}: Input/output error\\n' >&2\nexit 2\n"
            ),
        )
        .unwrap();
        fs::set_permissions(path, fs::Permissions::from_mode(0o700)).unwrap();
    }

    #[test]
    fn second_fixed_candidate_is_used_when_the_first_is_absent() {
        let directory = tempdir().unwrap();
        let missing = directory.path().join("missing");
        let second = directory.path().join("dcraw_emu");
        valid_script(&second);
        assert_eq!(
            resolve_trusted(&[missing, second.clone()], |_| true),
            Some(second)
        );
    }

    #[test]
    fn handshake_rejects_an_impostor() {
        let directory = tempdir().unwrap();
        let impostor = directory.path().join("dcraw_emu");
        fs::write(&impostor, "#!/bin/sh\nexit 0\n").unwrap();
        fs::set_permissions(&impostor, fs::Permissions::from_mode(0o700)).unwrap();
        assert!(!functional_handshake(&impostor));
    }

    #[test]
    fn handshake_kills_same_group_survivors_and_bounds_detached_pipes() {
        let directory = tempdir().unwrap();
        let survivor_pid = directory.path().join("survivor-pid");
        let same_group = directory.path().join("same-group");
        fs::write(
            &same_group,
            format!(
                "#!/bin/sh\nprintf 'Using 4 threads\\nProcessing file {PROBE_SENTINEL}\\nCannot open {PROBE_SENTINEL}\\n'\nsleep 5 >/dev/null 2>&1 & echo $! > '{}'\nexit 2\n",
                survivor_pid.display()
            ),
        )
        .unwrap();
        fs::set_permissions(&same_group, fs::Permissions::from_mode(0o700)).unwrap();
        assert!(functional_handshake(&same_group));
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
        assert!(!functional_handshake(&detached));
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
}
