use cxx_qt_lib::QUrl;
use std::path::{Path, PathBuf};

pub(super) fn local_destination(url: &QUrl) -> Option<PathBuf> {
    url.to_local_file()
        .map(|path| PathBuf::from(path.to_string()))
}

pub(super) fn display_file_name(path: &Path) -> &str {
    path.file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("photograph")
}
