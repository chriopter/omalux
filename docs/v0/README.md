<!-- Archived Omalux v0 documentation. Commands and plain source paths are relative to omalux-v0/ unless stated otherwise. -->

# Omalux v0

The original implementation, preserved while [Omalux moves toward darktable](../../README.md). Run the commands below from `omalux-v0/`.

A focused photo developer for Omarchy, built with Rust and Qt Quick.
Open RAW, JPEG, PNG, or BMP images, adjust color and tone, apply presets and
film grain, and export JPEG or HEIC. Preview and export share one CPU pipeline.

**In development:** UI and presets are being polished.

The website source and images live in [omalux.org/](../website.md).

![Omalux — RAW photo developer](../../omalux.org/public/app-screenshot-dark.png)

## Details & development

The desktop app follows your Omarchy theme and supports keyboard-driven editing.
Press `?` for shortcuts. A separate Qt-free CLI exposes the same processing core.
Presets include [stored beach-scene previews and reference checks](preset-previews.md).

Panels: **1** Filters, **2** Presets, **3** Crop & Rotate, **4** Metadata.
Drag the crop frame and its handles; choose a free or fixed aspect ratio.
Rotation previews update immediately; arrows adjust 0.1°, Shift + arrows 5°.
Enter applies the crop, Escape cancels. Switching tools applies it.
**Save as preset…** stores a personal look with a generated beach-scene thumbnail.
Find it under **My Presets**, with rename, update, JSON export and delete actions.
Personal looks omit geometry and local masks, preserving those edits when applied.
Data lives in `$XDG_DATA_HOME/omalux/` (default `~/.local/share/omalux/`):
one shared reference image and `presets/user-<id>/{preset.json,thumbnail.jpg}`.

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

See [architecture](architecture.md), [CLI usage](cli.md),
[HEIC support](io-heic-encode.md), [film grain](grain-model.md),
and [contributor instructions](../../AGENTS.md#archived-engine-rules-omalux-v0).

Releases mark completed, tested feature batches; small fixes can be collected
between releases. See [GitHub Releases](https://github.com/chriopter/omalux/releases)
for release notes and downloads.

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

[Asset notes and generator](reference-pictures.md) ·
[Exact values and hashes](../../omalux-v0/reference%20pictures/technical/manifest.json)
