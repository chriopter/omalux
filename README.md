# Omalux

A focused photo developer for Omarchy, built with Rust and Qt Quick.
Open RAW, JPEG, PNG, or BMP images, adjust color and tone, apply presets and
film grain, and export JPEG or HEIC. Preview and export share one CPU pipeline.

**In development:** UI and presets are being polished.

The website source and images live in [omalux.org/](omalux.org/README.md).

![Omalux — RAW photo developer](omalux.org/public/app-screenshot-dark.png)

## Details & development

The desktop app follows your Omarchy theme and supports keyboard-driven editing.
Press `?` for shortcuts. A separate Qt-free CLI exposes the same processing core.
Presets include [stored beach-scene previews and reference checks](docs/preset-previews.md).

**Requirements:** Rust, Little CMS 2, and LibRaw for RAW decoding. The desktop
also needs Qt 6 Quick/Quick Controls, a C++ compiler, ImageMagick, and libheif/x265
for HEIC export (enabled by default in the GUI).

```bash
# Run the desktop app
cargo run --release -p omalux-gui

# Develop a photo with the CLI
cargo run --release -p omalux -- develop \
  --input photo.jpg --output result.jpg --preset neutral
```

Add `--features heic` to the CLI build for HEIC export.

Before committing:

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo test -p omalux --features heic
```

See [architecture](docs/architecture.md), [CLI usage](docs/cli.md),
[HEIC support](docs/io-heic-encode.md), [film grain](docs/grain-model.md),
and [contributor instructions](AGENTS.md).

## Example pictures

An AI-generated beach scene and six reproducible 16-bit sRGB test charts.
The beach scene powers preset thumbnails and pixel-exact reference checks.
The technical charts are intended for automated calibration and color/tone checks;
their automation is still to be added.
Click a thumbnail to open the full-size image.

<table>
  <tr>
    <td align="center"><a href="reference%20pictures/main.jpg"><img src="reference%20pictures/main.jpg" width="220" alt="Beach scene"></a><br>Beach scene</td>
    <td align="center"><a href="reference%20pictures/technical/01-grayscale.png"><img src="reference%20pictures/technical/01-grayscale.png" width="220" alt="Grayscale"></a><br>Grayscale</td>
    <td align="center"><a href="reference%20pictures/technical/02-shadows-highlights.png"><img src="reference%20pictures/technical/02-shadows-highlights.png" width="220" alt="Shadows &amp; highlights"></a><br>Shadows &amp; highlights</td>
  </tr>
  <tr>
    <td align="center"><a href="reference%20pictures/technical/03-channel-ramps.png"><img src="reference%20pictures/technical/03-channel-ramps.png" width="220" alt="Color channels"></a><br>Color channels</td>
    <td align="center"><a href="reference%20pictures/technical/04-hue-saturation.png"><img src="reference%20pictures/technical/04-hue-saturation.png" width="220" alt="Hue &amp; saturation"></a><br>Hue &amp; saturation</td>
    <td align="center"><a href="reference%20pictures/technical/05-color-patches.png"><img src="reference%20pictures/technical/05-color-patches.png" width="220" alt="Color patches"></a><br>Color patches</td>
  </tr>
  <tr>
    <td align="center"><a href="reference%20pictures/technical/06-spatial-detail.png"><img src="reference%20pictures/technical/06-spatial-detail.png" width="220" alt="Detail &amp; noise"></a><br>Detail &amp; noise</td>
  </tr>
</table>

[Asset notes and generator](reference%20pictures/README.md) ·
[Exact values and hashes](reference%20pictures/technical/manifest.json)
