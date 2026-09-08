# Release notes

## 0.2.0 — 2026-09-08

- Save personal presets with names and automatically rendered beach-scene thumbnails.
  Presets are stored locally, support explicit replacement or copies, and can be
  renamed, updated, exported as JSON and deleted. Saving runs in the background;
  JSON and thumbnail are published together. Geometry and local masks stay with
  the photograph.
- Add Crop & Rotate after Presets: four crop margins, clockwise/counterclockwise
  quarter turns, fine straightening and reset. Shortcuts 1–4 select the tools.
- Preserve zoom and image position across effect previews; apply zoom gestures
  immediately and keep gesture panning within image bounds.
- Add slider, category and global filter resets. Start with grain at zero.
- Show uppercase image extensions in the file picker.
- Compact preset previews, accordion groups, central keyboard shortcuts and
  fractional exposure controls.

Linux builds require the system libraries documented in README.md. The crop tool
currently uses percentage margins rather than an on-image crop rectangle.
