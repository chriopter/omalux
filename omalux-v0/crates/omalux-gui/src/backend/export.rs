use cxx_qt_lib::QUrl;
use std::path::{Path, PathBuf};

pub(super) fn local_destination(url: &QUrl) -> Option<PathBuf> {
    url.to_local_file()
        .map(|path| absolute_local_path(Path::new(&path.to_string())))
}

pub(super) fn absolute_local_path(path: &Path) -> PathBuf {
    if path.is_absolute() {
        path.to_owned()
    } else {
        std::env::current_dir()
            .map(|directory| directory.join(path))
            .unwrap_or_else(|_| path.to_owned())
    }
}

pub(super) fn display_file_name(path: &Path) -> &str {
    path.file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("photograph")
}
