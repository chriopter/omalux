# Qt-free command line

`grainroom` is the Qt-free process interface. Argument parsing completes before
any command is dispatched. Help, version, syntax errors, range errors, and the
legacy `--headless` option therefore cannot launch the desktop application.

## Commands

```text
grainroom gui [--input PATH]
grainroom develop --input PATH --output PATH
                  [--format jpeg|jpg|heic|heif] [--quality 1..100]
                  [--preset ID | --preset-file PATH] [--set ID=VALUE]...
                  [--unprofiled assume-srgb|reject]
                  [--metadata preserve-safe|strip-location|strip-all]
                  [--alpha reject|flatten-black|flatten=#RRGGBB]
                  [--overwrite] [--json] [--progress none|human|json]
grainroom presets list [--json]
grainroom presets show ID [--json]
grainroom parameters list [--json]
grainroom probe [--json]
```

`gui` is the sole GUI-launching command. It derives `grainroom-gui` from the
directory of the running `grainroom` executable and passes an input path as the
two arguments `--input`, `PATH`. It never resolves the GUI through `PATH`.
On Linux it holds the executable directory, opens the sibling with
`O_NOFOLLOW`, verifies a regular executable, and launches that held file via
`/proc/self/fd`; replacing the sibling name after validation cannot redirect
the launch. The containing installation directory remains the packaging trust
boundary: administrators must not allow untrusted users to create executable
hardlinks there.
Paths remain native `PathBuf`/`OsString` values, so non-UTF-8 Linux filenames
are not converted or logged.

`develop` resolves a built-in preset and validates all typed overrides as one
transaction. `--preset` and `--preset-file` are mutually exclusive, and
duplicate `--set` parameter IDs are rejected. An external preset path is kept
opaque until production services are wired, so this validation phase performs
no file I/O. Output format is
inferred case-insensitively from `.jpg`, `.jpeg`, `.heic`, or `.heif` when
`--format` is absent. The command does not inspect the input or destination at
this stage. The typed job boundary exists, but a valid request remains
unavailable until production decoder and encoder services are wired; no
destination is created.
Because execution is unavailable, every `--progress` mode currently emits no
progress events. JSON mode emits only the final unavailable object on stdout;
human mode emits only the final diagnostic on stderr.

Catalog, registry, and probe commands produce human-readable stdout by default
(`list` output is TSV) and compact JSON with `--json`. Probe considers only
fixed system/package paths owned by root and not group/world writable, then
runs a bounded `dcraw_emu` behavior handshake. It never searches `PATH`.
That ownership/mode check is the executable trust boundary; the handshake is
not a sandbox for arbitrary programs. Capture pipes are nonblocking and
bounded, the original process-group leader stays unreaped while same-group
survivors are killed, and even a detached process holding a pipe cannot delay
the probe beyond its deadline.
Probe JSON reports only the backend name and functional availability, never
executable paths. A JSON-mode unavailable develop result is
also path-free JSON on stdout. Other diagnostics are human-readable on stderr
and avoid echoing input/output paths.

## Exit status

- `0`: success, help, or version;
- `2`: usage, range, unknown preset, duplicate override, or invalid format;
- `69`: a validated operation is currently unavailable, including a missing
  packaged GUI sibling or the not-yet-wired develop executor;
- `70`: internal failure.

When `gui` starts successfully, the child application's ordinary 0–255 exit
status is propagated.
