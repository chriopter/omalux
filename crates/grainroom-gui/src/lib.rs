mod backend;

/// Keeps the CXX-Qt backend and generated QML registration linked into the GUI
/// executable without exposing Qt through the `grainroom` core crate.
#[doc(hidden)]
pub fn initialize_backend_types() {}
