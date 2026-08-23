use std::{
    fs,
    process::{Child, Command, ExitStatus},
    thread,
    time::{Duration, Instant},
};

fn wait_bounded(mut child: Child) -> ExitStatus {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        if let Some(status) = child.try_wait().unwrap() {
            return status;
        }
        if Instant::now() >= deadline {
            child.kill().unwrap();
            let _ = child.wait();
            panic!("headless GUI did not terminate within ten seconds");
        }
        thread::sleep(Duration::from_millis(20));
    }
}

#[test]
fn corrupt_jpeg_exits_nonzero_without_publishing_or_hanging() {
    let directory = tempfile::tempdir().unwrap();
    let input = directory.path().join("corrupt.jpg");
    let output = directory.path().join("output.jpg");
    fs::write(&input, b"not a jpeg").unwrap();
    let child = Command::new(env!("CARGO_BIN_EXE_grainroom-gui"))
        .env("QT_QPA_PLATFORM", "offscreen")
        .args(["--headless", "--input"])
        .arg(&input)
        .args(["--output"])
        .arg(&output)
        .args(["--format", "jpeg"])
        .spawn()
        .unwrap();
    let status = wait_bounded(child);
    assert!(!status.success());
    assert!(!output.exists());
}

#[test]
fn relative_jpeg_paths_resolve_against_the_process_working_directory() {
    let directory = tempfile::tempdir().unwrap();
    let input = directory.path().join("input.jpg");
    let output = directory.path().join("output.jpg");
    image::RgbImage::from_pixel(2, 2, image::Rgb([80, 120, 160]))
        .save(&input)
        .unwrap();
    let child = Command::new(env!("CARGO_BIN_EXE_grainroom-gui"))
        .current_dir(directory.path())
        .env("QT_QPA_PLATFORM", "offscreen")
        .args([
            "--headless",
            "--input",
            "input.jpg",
            "--output",
            "output.jpg",
            "--format",
            "jpeg",
        ])
        .spawn()
        .unwrap();
    assert!(wait_bounded(child).success());
    assert!(output.is_file());
    assert_eq!(image::image_dimensions(output).unwrap(), (2, 2));
}
