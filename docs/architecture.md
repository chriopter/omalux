# Architecture

Grainroom keeps application logic, interface tools, reusable controls, and GPU
programs separate so each part can evolve independently.

## Rust

- `src/main.rs` starts Qt and loads the QML application.
- `src/cli.rs` handles process-level command-line behavior.
- `src/backend/mod.rs` exposes the CXX-Qt API used by QML.
- `src/backend/loader.rs` loads standard images and develops RAW previews.
- `src/backend/metadata.rs` reads file, EXIF, and LibRaw metadata.
- `src/backend/export.rs` writes original, JPEG, and HEIC output.
- `src/backend/theme.rs` follows the active Omarchy theme.

## QML

- `qml/Main.qml` owns the application shell, navigation, and shared state.
- `qml/components/` contains reusable interface controls.
- `qml/tools/` contains one directory per editing or inspection tool.
- `qml/shaders/` contains only human-authored GPU source files.

Each shader stage has one source file. `build.rs` compiles those sources into
Qt Shader Baker packages under Cargo's build directory and embeds them into the
application. Generated `.qsb` files never live in the source tree.
