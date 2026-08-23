use std::path::Path;

pub(super) fn is_qt_image(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .map(|extension| {
            matches!(
                extension.to_ascii_lowercase().as_str(),
                "jpg" | "jpeg" | "png" | "bmp"
            )
        })
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::is_qt_image;
    use std::path::Path;

    #[test]
    fn routes_standard_images_directly_to_qt() {
        assert!(is_qt_image(Path::new("portrait.JPG")));
        assert!(is_qt_image(Path::new("scan.png")));
        assert!(is_qt_image(Path::new("reference.bmp")));
    }

    #[test]
    fn routes_camera_raw_files_to_libraw() {
        assert!(!is_qt_image(Path::new("capture.dng")));
        assert!(!is_qt_image(Path::new("capture.nef")));
        assert!(!is_qt_image(Path::new("capture.cr3")));
    }
}
