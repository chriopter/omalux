# Grainroom

A focused photo developer for Omarchy, built with Rust, CXX-Qt, Qt Quick and LibRaw.

Implementation boundaries and directory responsibilities are documented in
[`docs/architecture.md`](docs/architecture.md). The film-grain implementation
and licensing provenance live in [`docs/grain-model.md`](docs/grain-model.md).

## MVP

- Open JPEG, PNG, BMP and common camera RAW files
- Decode a half-size RAW preview with LibRaw
- Apply procedural monochrome grain live with a Qt Quick fragment shader
- Follow the active Omarchy color theme live, including theme switches while running
- Switch between Crop (placeholder), Grain, and real file/EXIF metadata panels
- Keep Grain as the primary control, with Size and Midtones in a collapsible advanced section; five labeled mock controls demonstrate sidebar growth
- Save the unchanged original or export JPEG/HEIC at adjustable quality, with resolution and size estimates
- Zoom from 25% to 800% with the mouse wheel or touchpad pinch, and pan enlarged photographs
- Navigate the TUI-inspired develop panel with J/K or arrows, adjust with H/L, and press `?` for the complete keyboard reference
- Use the GPL-compatible darktable/RawTherapee three-octave grain and photographic-paper response model

## Keyboard

- `1`, `2`, `3`: open Crop, Grain, or Metadata; `Tab` and `[`/`]` cycle panels
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
grainroom --input ~/Pictures/photo.jpg
```

Run the same Qt Quick grain shader and HEIC export path without dialogs:

```bash
grainroom --headless \
  --input ~/Pictures/photo.jpg \
  --output /tmp/photo-grainroom.heic \
  --format heic \
  --quality 90 \
  --grain 24 \
  --grain-size 4000 \
  --midtones 100
```

`--format` accepts `original`, `jpg`/`jpeg`, and `heic`/`heif`. The process
returns a non-zero exit status when opening, rendering, or encoding fails.

After building, run the automated end-to-end export check with:

```bash
scripts/smoke-cli-export.sh ~/Downloads/example.jpg
```

## Requirements

- Rust
- Qt 6 with Qt Quick, Qt Quick Controls and Shader Tools
- LibRaw (`dcraw_emu` is used by the MVP)
- ImageMagick (`magick identify` reads metadata and encodes JPEG/HEIC exports)
- A C++ compiler

## Run

```bash
cargo run --release
```

Qt Shader Baker compiles `qml/shaders/film_grain.frag` during the build. The generated
`.qsb` package is embedded into the application and works with Qt's supported
graphics backends.
