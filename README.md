# Omalux

A keyboard-friendly interface for [darktable](https://www.darktable.org/), built for Omarchy. Our work is the UI; darktable provides the photo engine. Thanks to its developers and contributors. This is an independent project, not an official darktable edition.

[Website](https://omalux.org) · [Releases](https://github.com/chriopter/omalux/releases) · [Documentation](docs/README.md)

![Omalux development preview](omalux.org/public/app-screenshot-dark.png)

## Run

**Early development, source only.** Requires darktable 5.6.0 or 5.6.1, Qt 6 and build tools. See [setup requirements](docs/development.md#development-scripts).

```sh
git clone --recurse-submodules https://github.com/chriopter/omalux.git
cd omalux
bin/dev
```

Sessions are temporary: export your photos or save presets before closing.

## Development scripts

- `bin/dev [image]` — start Omalux; defaults to the beach photo.
- `bin/dev_split [image]` — also open darktable for comparison.
- `bin/preset_preview <folder>` / `--all` — regenerate preset thumbnails.
- `bin/update` — update the darktable submodule to the latest stable release; does not commit or push.

[Development guide](docs/development.md) · [Engine architecture](docs/darktable-architecture.md) · [Original v0 app](docs/v0/README.md)

## TODO

- [ ] Calibrate presets against their intended appearance ([details](docs/presets.md#one-time-v0-conversion)).
