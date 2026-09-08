# Preset pictures

Each built-in look keeps its settings and pictures together:

```text
presets/builtin/film/film-grain/
├── preset.json
├── thumbnail.jpg
└── reference.json
```

`preset.json` is canonical compact JSON plus one newline. `reference.json`
stores the SHA-256 hash of the full-resolution rendering of the shared
`reference pictures/main.jpg`, plus its dimensions and pixel format.
`thumbnail.jpg` is derived from that rendering, with a 288-pixel long edge,
for the preset list. No full-size output images are stored.

The GUI embeds the stored thumbnails as Qt resources. Browsing does not render
anything or change the selected preset, and works without an open photograph.
Applying a preset requires a photograph. The previews illustrate each look on
the beach scene, not its result on the user's current image.

## Generate and verify

Create or deliberately update hashes and thumbnails after changing a preset:

```bash
cargo run --release --example preset_references -- --write film-grain
```

Omit the ID to update all presets. Review changes before accepting new hashes.
Normal builds and checks never replace the stored references.

Run the reference check before a release or as a CI build gate:

```bash
cargo run --release --example preset_references -- --check
cargo build --release -p omalux-gui
```

Both modes use the same production decoder, full development pipeline and
working-space-to-sRGB transform, with the source digest supplying deterministic
grain. Full-resolution images exist only in memory during generation and checking.

`--check` compares the version, dimensions, pixel format and SHA-256 hash. The
hash covers every quantized 16-bit sRGB sample in row-major RGB order, encoded
as two big-endian bytes per sample (`srgb-rgb16be`), with no padding or header.
This includes low bits and is independent of PNG/JPEG compression or metadata.
The stored thumbnail bytes are checked too.

A mismatch exits nonzero without overwriting either file. Exact comparisons
can expose toolchain or color-library changes as well as renderer changes;
review those rather than automatically accepting a new baseline. Hashes detect
changes but cannot reconstruct the previous image for a visual difference map.
This tests the raster development path, not RAW decoding or lossy export codecs.

## Implementation

The explicit manifest in `src/preset/catalog.rs` lists each directory once.
It supplies both catalogue JSON and, via the opt-in `preset-thumbnails` feature,
the GUI build's thumbnail bytes. Reference hashes are never embedded in the app.
This works with separately packaged crates without copying source assets into
the GUI package, scanning the filesystem at runtime, or recursively invoking
Cargo from a build script.

`cargo test -p omalux-gui --test preset_previews` loads all embedded thumbnails
offscreen, scrolls through the list, and checks preset selection. The catalogue
tests also validate the folder structure, hash metadata, and thumbnail dimensions.
