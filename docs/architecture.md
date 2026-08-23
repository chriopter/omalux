# Architecture

Grainroom keeps application logic, interface tools, reusable controls, and GPU
programs separate so each part can evolve independently.

## Workspace packages

- The root `grainroom` package is the Qt-free processing core and CLI. Its
  `src/develop/` and `src/io/` modules contain image math, decoding, color,
  metadata, and bounded I/O contracts. `src/main.rs` is intentionally a small
  process-level CLI entry point.
- `crates/grainroom-gui/` is the only Qt package. Its `src/main.rs` starts Qt,
  `src/backend/` exposes the CXX-Qt API used by QML, and its build script owns
  QML registration and shader baking. The installed executable is
  `grainroom-gui`.

The core package has no build script and no `cxx`, `cxx-qt`, `cxx-qt-lib`,
`notify`, QML, or Qt Shader Baker dependency. This keeps batch processing and
future CLI packages buildable on machines without Qt.

## GUI Rust

- `crates/grainroom-gui/src/backend/loader.rs` loads standard images and develops RAW previews.
- `crates/grainroom-gui/src/backend/metadata.rs` reads file, EXIF, and LibRaw metadata.
- `crates/grainroom-gui/src/backend/export.rs` writes original, JPEG, and HEIC output.
- `crates/grainroom-gui/src/backend/theme.rs` follows the active Omarchy theme.

## QML

- `crates/grainroom-gui/qml/Main.qml` owns the application shell, navigation, and shared state.
- `crates/grainroom-gui/qml/components/` contains reusable interface controls.
- `crates/grainroom-gui/qml/tools/` contains one directory per editing or inspection tool.
- `crates/grainroom-gui/qml/shaders/` contains only human-authored GPU source files.

Each shader stage has one source file. `crates/grainroom-gui/build.rs` compiles
those sources into Qt Shader Baker packages under Cargo's build directory and
embeds them into the application. Generated `.qsb` files never live in the
source tree.

## Packaging paths

- Core/CLI binary: `target/{debug,release}/grainroom`
- Desktop GUI binary: `target/{debug,release}/grainroom-gui`
- Desktop entry: `packaging/arch/grainroom.desktop` (`Exec=grainroom-gui`)
- GUI-only source assets: `crates/grainroom-gui/qml/`
