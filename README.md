# Omalux

A keyboard-friendly interface for [darktable](https://www.darktable.org/), built for Omarchy. Our work is the UI; darktable provides the photo engine. Thanks to its developers and contributors. This is an independent project, not an official darktable edition.

[Website](https://omalux.org) · [Releases](https://github.com/chriopter/omalux/releases) · [Documentation](development/docs/README.md)

![Omalux development preview](https://omalux.org/app-screenshot-dark.png)

## Run

**Early development, source only.** Requires darktable 5.6.0 or 5.6.1, Qt 6 and build tools. See [setup requirements](development/docs/setup.md#development-scripts).

```sh
git clone --recurse-submodules https://github.com/chriopter/omalux.git
cd omalux
development/start
```

Sessions are temporary: export your photos or save styles before closing.

## Development

Start scripts live in `development/`, calibration tools in `development/tools/calibration/`, and documentation in `development/docs/`.

- `development/start [image]` — start Omalux; defaults to the beach photo.
- `development/start_split [image]` — also open darktable for comparison.
- `development/style_preview <folder>` / `--all` — regenerate style thumbnails.
- `development/update` — update the darktable submodule to the latest stable release; does not commit or push.

[Development guide](development/docs/setup.md) · [Engine architecture](development/docs/architecture/darktable.md) · [Original v0 release](https://github.com/chriopter/omalux/releases/tag/v0.2.0)

## TODO

- [ ] Calibrate styles against their intended appearance ([details](development/docs/reference/styles.md#one-time-v0-conversion)).
