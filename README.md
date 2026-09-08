# Omalux

A focused, keyboard-driven photo editor for Omarchy, moving toward a new interface powered by [darktable](https://www.darktable.org/).

## A new direction

We are building on the work of the darktable developers and contributors, whose RAW processing and photography tools make this direction possible. Thank you for making that work available as free software.

The plan is to track the official darktable repository as a pinned Git submodule, keep its source unchanged, and maintain the Omalux Qt/QML interface and adapter separately. Upstream updates will be adopted as complete revisions and tested against our integration.

This is an independent project, not an official darktable edition or an endorsement by its developers. At this stage, this repository does not yet contain the darktable submodule or the new adapter. A local prototype has demonstrated a persistent darktable process serving the Omalux interface; integration into this repository is next. There is no new darktable-based release to download yet.

## Repository layout

- [omalux-v0/](omalux-v0/README.md): the original Rust engine, Qt/QML app, CLI, presets, tests and development tools, preserved together.
- [omalux.org/](omalux.org/README.md): the website.

To run the original app:

```sh
cd omalux-v0
cargo run --release -p omalux-gui
```

See the v0 README for dependencies and validation commands. Existing website screenshots show the v0 interface, not a completed darktable integration.

## Upstream and licensing

[darktable](https://github.com/darktable-org/darktable) is free software under GNU GPL version 3 or later; individual components retain their respective notices and licenses. Omalux v0 declares GPL-3.0-or-later in its Cargo manifest.

When distributing a derivative, preserve applicable copyright and license notices, identify changes, and provide the corresponding source under the applicable GPL terms. Upstream authorship stays with its contributors. We intend to keep Omalux-specific work separate and offer generally useful improvements upstream.
