# Omalux

A keyboard-friendly interface for [darktable](https://www.darktable.org/), built for Omarchy. Our work is the UI; darktable provides the photo engine. Thanks to its developers and contributors. This is an independent project, not an official darktable edition.

[Website](https://omalux.org) · [Releases](https://github.com/chriopter/omalux/releases) · [Documentation](dev/docs/README.md)

![Omalux development preview](https://omalux.org/app-screenshot-dark.png)

## Run

**Early development, source only.** Requires darktable 5.6.0 or 5.6.1, Qt 6 and build tools. See [setup requirements](dev/docs/development/setup.md#development-scripts).

```sh
git clone --recurse-submodules https://github.com/chriopter/omalux.git
cd omalux
dev/scripts/start
```

Sessions are temporary: export your photos or save presets before closing.

## Development

Documentation, scripts and calibration tools live in `dev/`.

- `dev/scripts/start [image]` — start Omalux; defaults to the beach photo.
- `dev/scripts/start_split [image]` — also open darktable for comparison.
- `dev/scripts/preset_preview <folder>` / `--all` — regenerate preset thumbnails.
- `dev/scripts/update` — update the darktable submodule to the latest stable release; does not commit or push.

[Development guide](dev/docs/development/setup.md) · [Engine architecture](dev/docs/architecture/darktable.md) · [Original v0 release](https://github.com/chriopter/omalux/releases/tag/v0.2.0)

## TODO

- [ ] Calibrate presets against their intended appearance ([details](dev/docs/reference/presets.md#one-time-v0-conversion)).
