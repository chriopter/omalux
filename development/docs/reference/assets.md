# Assets

- `logo/omalux-logo.svg`: the existing Omalux wordmark copied from the website. `omalux-logo-light.svg` uses the UI foreground colour for dark backgrounds.
- `images/beach-volleyball.jpg`: the unchanged 1536 × 1024 AI-generated beach volleyball image from Omalux v0. Used when `development/start` receives no image argument, and offered in the application as **Example photograph** (the arrow beside Open and the empty state); the application finds it in the assets folder it is given, so a package ships this one file. It is not a measured colour reference.

See [the original image notes](https://github.com/chriopter/omalux/blob/1d5eaf6/docs/v0/reference-pictures.md) for provenance and the generation prompt. Reuse this file rather than regenerating it.

The five sidebar SVGs in `assets/icons/` are reused from the archived Omalux v0 UI (`edit`, `styles`, `crop`, `history`, `info`). Keep shared icon assets here rather than duplicating them in individual panels.

The toolbar icons (`open`, `export`, `compare`, `chevron-down`, `photo`) are drawn in the same 24 px outline style as the sidebar icons and tinted by the interface.
