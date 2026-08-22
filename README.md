# Grainroom

A focused photo developer for Omarchy, built with Rust, CXX-Qt, Qt Quick and LibRaw.

## MVP

- Open JPEG, PNG, BMP and common camera RAW files
- Decode a half-size RAW preview with LibRaw
- Apply procedural monochrome grain live with a Qt Quick fragment shader
- Switch between Crop (placeholder), Grain, and real file/EXIF metadata panels
- Keep Grain as the primary control, with Size and Midtones in a collapsible advanced section; five labeled mock controls demonstrate sidebar growth
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
- `-`/`+`: zoom; `0`: fit; `O`: open; `?` or `F1`: keyboard reference
- Standard `Ctrl+O`, `Ctrl+-`, `Ctrl++`, and `Ctrl+0` shortcuts remain available

## Requirements

- Rust
- Qt 6 with Qt Quick, Qt Quick Controls and Shader Tools
- LibRaw (`dcraw_emu` is used by the MVP)
- ImageMagick (`magick identify` reads metadata from standard images)
- A C++ compiler

## Run

```bash
cargo run --release
```

Qt Shader Baker compiles `qml/shaders/grain.frag` during the build. The generated
`.qsb` package is embedded into the application and works with Qt's supported
graphics backends.
