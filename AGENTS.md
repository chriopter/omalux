# Working on Omalux

The repository is being prepared for an independent Qt/QML interface using darktable.

- `omalux-v0/` contains the original application and Rust engine. Read its `AGENTS.md` before changing it. Run Cargo commands from that directory.
- `omalux.org/` contains the website and its own contributor instructions.
- `darktable/` is the official upstream Git submodule, pinned to a stable release. Initialize it with `git submodule update --init --recursive`.
- `omalux/` contains the Qt/QML UI, C/C++ native adapter and Python development launcher. Brightness, contrast and saturation are connected to darktable; other editing tools remain placeholders. Do not describe planned functionality as released.
- Keep the adapter and UI separate from upstream. Necessary internal engine changes are authorized, but keep them minimal and documented.
- The native adapter uses internal structures: always compile against the exact installed darktable release headers. Never silently reuse a binary after a library upgrade.
- `assets/logo/` and `assets/images/` contain shared visual assets. Reuse the existing beach photograph.
- Credit darktable and its contributors. Do not imply official affiliation or endorsement.
- Website screenshots currently depict v0 and must be labelled accordingly.

- `bin/update` checks out the latest upstream stable release and its dependencies; it never stages, commits or pushes.
