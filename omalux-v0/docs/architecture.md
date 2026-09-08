# Architecture

Omalux keeps application logic, interface tools, and reusable controls
separate so each part can evolve independently.

## Workspace packages

- The root `omalux` package is the Qt-free processing core and CLI. Its
  `src/develop/` and `src/io/` modules contain image math, decoding, color,
  metadata, and bounded I/O contracts. Built-in preset JSON under
  `presets/builtin/` is compiled into this package and validated canonically.
  `src/main.rs` is intentionally a small process-level CLI entry point.
- `crates/omalux-gui/` is the only Qt package. Its `src/main.rs` starts Qt,
  `src/backend/` exposes the CXX-Qt API used by QML, and its build script owns
  QML registration. The installed executable is `omalux-gui`.

The core package has no build script and no `cxx`, `cxx-qt`, `cxx-qt-lib`,
`notify` or QML dependency. This keeps batch processing and
future CLI packages buildable on machines without Qt.

## GUI Rust

- `crates/omalux-gui/src/backend/develop.rs` adapts revisioned GUI requests to the shared production `DevelopJob` path.
- `crates/omalux-gui/src/backend/loader.rs` classifies sources for supplemental metadata display.
- `crates/omalux-gui/src/backend/metadata.rs` reads file, EXIF, and LibRaw metadata.
- `crates/omalux-gui/src/backend/export.rs` normalizes local GUI paths; production photo output remains in the core.
- `crates/omalux-gui/src/backend/theme.rs` follows the active Omarchy theme.

## QML

- `crates/omalux-gui/qml/Main.qml` owns the application shell, navigation, and shared state.
- `crates/omalux-gui/qml/components/` contains reusable interface controls.
- `crates/omalux-gui/qml/tools/` contains one directory per editing or inspection tool.

The GUI contains no alternate photo shader or rendered-export path. Preview
and export both consume the CPU pipeline so a visible control cannot silently
select different processing math.

## Packaging paths

- Core/CLI binary: `target/{debug,release}/omalux`
- Desktop GUI binary: `target/{debug,release}/omalux-gui`
- Desktop entry: `packaging/arch/omalux.desktop` (`Exec=omalux-gui %f`)
- GUI-only source assets: `crates/omalux-gui/qml/`

Both Cargo packages are independently packageable. The GUI manifest binds its
workspace path dependency to the exact same released core version. Installers
must place both `omalux` and `omalux-gui` in the executable search path
and install the desktop entry below `share/applications/`; only the GUI package
requires Qt at build and runtime.

`cargo package --workspace` verifies the two archives together through Cargo's
temporary registry. Core verification therefore compiles the embedded preset
catalog from the packaged `presets/builtin/` files, while the GUI archive is
verified against the exact packaged core version.
