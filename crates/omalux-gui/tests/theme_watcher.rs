#![cfg(unix)]

use std::{
    fs,
    io::{BufRead, BufReader},
    os::unix::fs::symlink,
    path::Path,
    process::{Child, Command, Stdio},
    sync::mpsc::{self, Receiver, RecvTimeoutError},
    thread,
    time::Duration,
};

struct RunningGui {
    child: Child,
    reloads: Receiver<String>,
    reader: Option<thread::JoinHandle<()>>,
}

impl Drop for RunningGui {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
        if let Some(reader) = self.reader.take() {
            let _ = reader.join();
        }
    }
}

fn theme_contents(background: &str) -> String {
    format!(
        "background = '{background}'\nforeground = '#eeeeee'\naccent = '#5584aa'\nselection = '#263746'\n"
    )
}

fn write_theme(directory: &Path, background: &str) {
    fs::create_dir_all(directory).unwrap();
    fs::write(directory.join("colors.toml"), theme_contents(background)).unwrap();
}

fn launch(home: &Path) -> RunningGui {
    let mut child = Command::new(env!("CARGO_BIN_EXE_omalux-gui"))
        .env("HOME", home)
        .env("QT_QPA_PLATFORM", "offscreen")
        .env("OMALUX_THEME_WATCH_TRACE", "1")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let stderr = child.stderr.take().unwrap();
    let (sender, reloads) = mpsc::channel();
    let reader = thread::spawn(move || {
        for line in BufReader::new(stderr).lines().map_while(Result::ok) {
            if line.starts_with("omalux-theme-reload:") {
                let _ = sender.send(line);
            }
        }
    });
    RunningGui {
        child,
        reloads,
        reader: Some(reader),
    }
}

fn expect_one_reload(gui: &RunningGui, background: &str) {
    assert_eq!(
        gui.reloads.recv_timeout(Duration::from_secs(3)).unwrap(),
        format!("omalux-theme-reload:{background}")
    );
    assert!(matches!(
        gui.reloads.recv_timeout(Duration::from_millis(300)),
        Err(RecvTimeoutError::Timeout)
    ));
}

#[test]
fn atomic_theme_mutations_are_live_coalesced_and_keep_the_stable_parent_watch() {
    let home = tempfile::tempdir().unwrap();
    let state = home.path().join(".local/state/omarchy");
    let current = state.join("current");
    write_theme(&current.join("theme-a"), "#101011");
    write_theme(&current.join("theme-b"), "#202022");
    symlink("theme-a", current.join("theme")).unwrap();

    let gui = launch(home.path());
    thread::sleep(Duration::from_millis(750));
    assert!(matches!(
        gui.reloads.try_recv(),
        Err(mpsc::TryRecvError::Empty)
    ));

    let colors = current.join("theme-a/colors.toml");
    let replacement = current.join("theme-a/colors.toml.next");
    fs::write(&replacement, theme_contents("#303033")).unwrap();
    fs::rename(replacement, colors).unwrap();
    expect_one_reload(&gui, "#303033");

    let next_link = current.join("theme.next");
    symlink("theme-b", &next_link).unwrap();
    fs::rename(next_link, current.join("theme")).unwrap();
    expect_one_reload(&gui, "#202022");

    let next_current = state.join("current.next");
    write_theme(&next_current.join("theme-c"), "#404044");
    symlink("theme-c", next_current.join("theme")).unwrap();
    fs::rename(&current, state.join("current.old")).unwrap();
    fs::rename(&next_current, &current).unwrap();
    expect_one_reload(&gui, "#404044");
}
