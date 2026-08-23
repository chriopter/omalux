# Grainroom

A focused photo developer for Omarchy. The workspace separates a Qt-free Rust
processing core/CLI from the CXX-Qt and Qt Quick desktop application.

Implementation boundaries and directory responsibilities are documented in
[`docs/architecture.md`](docs/architecture.md). The film-grain implementation
and licensing provenance live in [`docs/grain-model.md`](docs/grain-model.md).

## MVP

- Open JPEG, PNG, BMP and common camera RAW files
- Decode a half-size RAW preview with LibRaw
- Apply procedural monochrome grain live with a Qt Quick fragment shader
- Follow the active Omarchy color theme live, including theme switches while running
- Switch between Crop, Edit, Presets, and real file/EXIF metadata panels in a compact two-row tool grid
- Edit groups all controls under Basics, Color, and Effects; Grain remains live, with Size and Midtones in a collapsible advanced section
- Try three generic preset mockups while the preset processing pipeline is still being built
- Save the unchanged original or export JPEG/HEIC at adjustable quality, with resolution and size estimates
- Zoom from 25% to 800% with the mouse wheel or touchpad pinch, and pan enlarged photographs
- Navigate the TUI-inspired develop panel with J/K or arrows, adjust with H/L, and press `?` for the complete keyboard reference
- Use the GPL-compatible darktable/RawTherapee three-octave grain and photographic-paper response model

## Keyboard

- `1`–`4`: open Crop, Edit, Presets, or Metadata; `Tab` and `[`/`]` cycle panels
- `J`/`K` or `Down`/`Up`: select a parameter
- `H`/`L` or `Left`/`Right`: adjust it; hold `Shift` for larger steps
- `G`, `S`, `M`: jump to Grain, Size, or Midtones
- `A`: expand or collapse the Grain subparameters
- `R`: reset the selected parameter; `B`: toggle grain bypass
- `-`/`+`: zoom; `0`: fit; `F`: image-only fullscreen (any key exits); `O`: open; `?` or `F1`: keyboard reference
- `Ctrl+S`: choose Original, JPEG, or HEIC export
- Standard `Ctrl+O`, `Ctrl+-`, `Ctrl++`, and `Ctrl+0` shortcuts remain available

## Command line

Open a photograph directly:

```bash
grainroom-gui --input ~/Pictures/photo.jpg
```

Run the same Qt Quick grain shader and HEIC export path without dialogs:

```bash
grainroom-gui --headless \
  --input ~/Pictures/photo.jpg \
  --output /tmp/photo-grainroom.heic \
  --format heic \
  --quality 90 \
  --grain 24 \
  --grain-size 4000 \
  --midtones 100
```

The GUI command's `--format` accepts `original`, `jpg`/`jpeg`, and
`heic`/`heif`. The process
returns a non-zero exit status when opening, rendering, or encoding fails.

After building, run the automated end-to-end export check with:

```bash
scripts/smoke-cli-export.sh ~/Downloads/example.jpg
```

The Qt-free core executable exposes machine-testable catalog, parameter, and
backend commands without loading Qt:

```bash
grainroom --help
grainroom presets list
grainroom presets show neutral
grainroom parameters list
grainroom probe
```

Launch the separately packaged desktop sibling explicitly with
`grainroom gui [--input PATH]`. The core resolves `grainroom-gui` beside its own
executable and never searches `PATH` for it.

A development request can already be validated without touching its files:

```bash
grainroom develop photo.raw --output result.jpg \
  --preset neutral --set basics.contrast=10 --quality 90
```

Until the job/encoder boundary is connected, a valid request exits explicitly
as unavailable and never creates the output. See [`docs/cli.md`](docs/cli.md)
for the complete contract.

## Requirements

Core and CLI:

- Rust
- Little CMS 2
- LibRaw (`dcraw_emu` is used by RAW decoding)

Desktop GUI additionally requires:

- Qt 6 with Qt Quick, Qt Quick Controls and Shader Tools
- ImageMagick (`magick identify` reads metadata and encodes JPEG/HEIC exports)
- A C++ compiler

## Run

```bash
cargo run --release -p grainroom-gui
```

Build or test the Qt-free core alone with `cargo build -p grainroom` and
`cargo test -p grainroom`. Qt Shader Baker is invoked only by the
`grainroom-gui` package for
`crates/grainroom-gui/qml/shaders/film_grain.frag`; the generated `.qsb`
package is embedded into the desktop application.

## Install and package

Install both executables from a checkout with matching package versions:

```bash
cargo install --path . --locked
cargo install --path crates/grainroom-gui --locked
```

The first command installs the Qt-free `grainroom` core/CLI; the second installs
the `grainroom-gui` desktop application and therefore requires Qt. Install and
validate the desktop entry separately, for example for the current user:

```bash
install -Dm644 packaging/arch/grainroom.desktop \
  ~/.local/share/applications/grainroom.desktop
desktop-file-validate ~/.local/share/applications/grainroom.desktop
```

Distribution packages must ship both binaries in the executable search path
and install `packaging/arch/grainroom.desktop` under
`share/applications/grainroom.desktop`. The desktop entry passes one selected
photo to `grainroom-gui` through `%f`.

Before publishing source packages, validate both Cargo package manifests and
their included files:

```bash
cargo package --allow-dirty --workspace
```

The GUI dependency pins the exact matching `grainroom` core version while also
retaining its workspace path for local builds. Packaging the workspace together
lets Cargo build and fully verify both archives against its temporary local
registry before either package has been published. Release the core package
before the GUI package so the same exact dependency can be resolved by the
target registry.
