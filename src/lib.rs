mod backend;
pub mod develop;

/// Keeps the CXX-Qt backend and its generated QML registration linked into the
/// application binary while exposing the develop foundation as a Rust library.
#[doc(hidden)]
pub fn initialize_backend_types() {}
