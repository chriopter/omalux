# Working on Omalux

The repository is being prepared for an independent Qt/QML interface using darktable.

- `omalux-v0/` contains the original application and Rust engine. Read its `AGENTS.md` before changing it. Run Cargo commands from that directory.
- `omalux.org/` contains the website and its own contributor instructions.
- `darktable/` is the official upstream Git submodule, pinned to a stable release. Initialize it with `git submodule update --init --recursive`.
- The new Omalux interface and production adapter are not integrated yet. Do not describe planned functionality as released.
- Keep upstream darktable source unchanged; keep the adapter and UI separate.
- Credit darktable and its contributors. Do not imply official affiliation or endorsement.
- Website screenshots currently depict v0 and must be labelled accordingly.

- `bin/update` checks out the latest upstream stable release and its dependencies; it never stages, commits or pushes.
