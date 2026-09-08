# Omalux

A focused photo editing interface for Omarchy, based on [darktable](https://www.darktable.org/). Our UI, darktable’s image processing.

[omalux.org](https://omalux.org)

![Omalux v0 interface](omalux.org/public/app-screenshot-dark.png)

*The original Omalux v0 interface; darktable integration is in development.*

## A new direction

We are building on the work of the darktable developers and contributors, whose RAW processing and photography tools make this direction possible. Thank you for making that work available as free software.

We track the official darktable repository as a pinned Git submodule, keep its source unchanged, and will maintain the Omalux Qt/QML interface and adapter separately. Upstream updates will be adopted as complete revisions and tested against our integration.

This is an independent project, not an official darktable edition or an endorsement by its developers. The darktable source and a minimal Hello World UI are included; the image-processing adapter is not integrated yet. A local prototype has demonstrated a persistent darktable process serving the Omalux interface; integration into this repository is next. There is no new darktable-based release to download yet.

## Repository layout

- [omalux/](omalux/): the new minimal Qt/QML interface and development launcher.
- [darktable/](darktable/): unchanged upstream source, pinned to a stable release by the Git submodule entry.
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

## Fetching darktable

For a new checkout:

```sh
git clone --recurse-submodules https://github.com/chriopter/omalux.git
```

For an existing checkout, run from the repository root:

```sh
git submodule update --init --recursive
```

The submodule points directly to the official upstream repository. Keep it at the recorded commit; its nested submodules are pinned by darktable. Updating this source does not install darktable or change the system application.

## Updating darktable

Requires Git and the GitHub CLI (`gh`). Run from the repository root:

```sh
bin/update
git diff --submodule=short
```

The script checks out the latest official stable release and its nested submodules. It stops if darktable has local changes. After trying the new version, pin it with `git add darktable` and commit. The script does not stage, commit or push.

## Development scripts

Run from the repository root:

- `bin/dev` — open the Hello World UI and keep darktable’s engine running without a window. Closing the UI stops the engine.
- `bin/dev_split` — open the Hello World UI and the normal darktable window separately. Closing either window stops the development session.
- `bin/update` — check out the latest stable darktable release and its dependencies; review and commit the new pin yourself.

The launchers require Python 3, Qt 6 with Qt Quick Controls and its `qml6`/`qml` runtime, and an installed darktable with Lua support. They currently use the **installed** darktable, not an automatic build of the submodule. `bin/dev` loads `libdarktable.so` in a separate process using the internal initialization API (tested with 5.6.0); this is a temporary development shim, not a stable public API. Override `DARKTABLE_LIBRARY` or `DARKTABLE_BIN` for another matching installation.

Each launch uses a temporary config, cache and database. There is no image editing or communication between the Hello World UI and darktable yet. These commands open windows on your current workspace.
