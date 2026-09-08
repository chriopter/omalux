use super::loader::is_qt_image;
use std::path::Path;
use std::process::Command;

pub(super) fn read_metadata(path: &Path) -> String {
    let mut rows = Vec::new();
    let extension = path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or("UNKNOWN")
        .to_ascii_uppercase();
    push_metadata(&mut rows, "FORMAT", &extension);

    if let Ok(metadata) = std::fs::metadata(path) {
        push_metadata(&mut rows, "FILE SIZE", &human_file_size(metadata.len()));
    }

    if is_qt_image(path) {
        read_imagemagick_metadata(path, &mut rows);
    } else {
        read_raw_metadata(path, &mut rows);
    }

    rows.join("\n")
}

fn read_imagemagick_metadata(path: &Path, rows: &mut Vec<String>) {
    const FORMAT: &str = "%m\x1f%wx%h\x1f%[EXIF:Make]\x1f%[EXIF:Model]\x1f%[EXIF:LensModel]\x1f%[EXIF:DateTimeOriginal]\x1f%[EXIF:ExposureTime]\x1f%[EXIF:FNumber]\x1f%[EXIF:ISOSpeedRatings]\x1f%[EXIF:FocalLength]";
    let Ok(output) = Command::new("magick")
        .args(["identify", "-quiet", "-format", FORMAT])
        .arg(path)
        .output()
    else {
        return;
    };
    if !output.status.success() {
        return;
    }

    let output = String::from_utf8_lossy(&output.stdout);
    let values: Vec<&str> = output.split('\x1f').collect();
    if values.len() < 10 {
        return;
    }

    push_metadata(rows, "CODEC", values[0]);
    push_metadata(rows, "DIMENSIONS", values[1]);
    let camera = [clean_metadata(values[2]), clean_metadata(values[3])]
        .into_iter()
        .filter(|value| !value.is_empty())
        .collect::<Vec<_>>()
        .join(" ");
    push_metadata(rows, "CAMERA", &camera);
    push_metadata(rows, "LENS", &clean_metadata(values[4]));
    push_metadata(rows, "CAPTURED", &clean_metadata(values[5]));
    push_metadata(rows, "SHUTTER", &clean_metadata(values[6]));
    push_metadata(rows, "APERTURE", &clean_metadata(values[7]));
    push_metadata(rows, "ISO", &clean_metadata(values[8]));
    push_metadata(rows, "FOCAL LEN", &clean_metadata(values[9]));
}

fn read_raw_metadata(path: &Path, rows: &mut Vec<String>) {
    let Ok(output) = Command::new("dcraw_emu")
        .args(["-i", "-v"])
        .arg(path)
        .output()
    else {
        return;
    };
    if !output.status.success() {
        return;
    }

    let output = String::from_utf8_lossy(&output.stdout);
    for (label, field) in [
        ("CAMERA", "Camera"),
        ("CAPTURED", "Timestamp"),
        ("ISO", "ISO speed"),
        ("SHUTTER", "Shutter"),
        ("APERTURE", "Aperture"),
        ("FOCAL LEN", "Focal length"),
        ("DIMENSIONS", "Image size"),
    ] {
        if let Some(value) = metadata_field(&output, field) {
            push_metadata(rows, label, value);
        }
    }
}

fn metadata_field<'a>(output: &'a str, name: &str) -> Option<&'a str> {
    output.lines().find_map(|line| {
        let (key, value) = line.split_once(':')?;
        (key.trim() == name).then_some(value.trim())
    })
}

fn clean_metadata(value: &str) -> String {
    value
        .trim()
        .trim_matches('"')
        .replace("Undefined", "")
        .trim()
        .to_owned()
}

fn push_metadata(rows: &mut Vec<String>, label: &str, value: &str) {
    let value = value.trim();
    if !value.is_empty() {
        rows.push(format!("{label:<12} {value}"));
    }
}

pub(super) fn human_file_size(bytes: u64) -> String {
    const UNITS: [&str; 4] = ["B", "KB", "MB", "GB"];
    let mut value = bytes as f64;
    let mut unit = 0;
    while value >= 1024.0 && unit < UNITS.len() - 1 {
        value /= 1024.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{} {}", bytes, UNITS[unit])
    } else {
        format!("{value:.1} {}", UNITS[unit])
    }
}

#[cfg(test)]
mod tests {
    use super::{human_file_size, metadata_field};

    #[test]
    fn formats_file_sizes_for_metadata_panel() {
        assert_eq!(human_file_size(512), "512 B");
        assert_eq!(human_file_size(1_572_864), "1.5 MB");
    }

    #[test]
    fn extracts_libraw_metadata_fields() {
        let output = "Camera: Fujifilm X-T5\nISO speed: 800\nShutter: 1/125 sec\n";
        assert_eq!(metadata_field(output, "Camera"), Some("Fujifilm X-T5"));
        assert_eq!(metadata_field(output, "ISO speed"), Some("800"));
    }
}
