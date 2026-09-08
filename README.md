# Omalux

A focused photo developer for Omarchy, built with Rust and Qt Quick.
Open RAW, JPEG, PNG, or BMP images, adjust color and tone, apply presets and
film grain, and export JPEG or HEIC. Preview and export share one CPU pipeline.

**In development:** UI and presets are being polished.

![Omalux — RAW photo developer](docs/assets/screenshot.png)

<details>
<summary>Details & development</summary>

The desktop app follows your Omarchy theme and supports keyboard-driven editing.
Press `?` for shortcuts. A separate Qt-free CLI exposes the same processing core.

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

</details>

<details>
<summary>Example pictures & technical references</summary>

Fixed inputs for visual comparisons: one AI-generated beach scene and six
programmatically generated, lossless 16-bit sRGB charts. These are test inputs,
not calibrated photographs or an automated regression suite.

**Beach scene**

![Beach volleyball, sand, sea, and sky](reference%20pictures/main.jpg)

**Grayscale**

![Continuous and stepped grayscale](reference%20pictures/technical/01-grayscale.png)

**Shadows & highlights**

![Near-black and near-white ramps](reference%20pictures/technical/02-shadows-highlights.png)

**Color channels**

![Six paired color ramps](reference%20pictures/technical/03-channel-ramps.png)

**Hue & saturation**

![Hue and saturation at three brightness levels](reference%20pictures/technical/04-hue-saturation.png)

**Color patches**

![Thirty-two known color patches](reference%20pictures/technical/05-color-patches.png)

**Detail & noise**

![Edges, bars, checkerboard, gray, and noise](reference%20pictures/technical/06-spatial-detail.png)

Inspect technical images at 100% zoom. Regenerate them with:

```bash
python3 "reference pictures/technical/generate.py"
```

[Asset notes and generation prompt](reference%20pictures/README.md) ·
[Exact values and hashes](reference%20pictures/technical/manifest.json)

</details>
