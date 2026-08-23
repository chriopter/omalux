# Qt-free command line

`grainroom` is the Qt-free process interface. Argument parsing completes before
any command is dispatched. Help, version, syntax errors, range errors, and the
legacy `--headless` option therefore cannot launch the desktop application.

## Commands

```text
grainroom gui [--input PATH]
grainroom develop INPUT --output PATH [--preset ID] [--set ID=VALUE]...
                  [--format jpeg|heic] [--quality 1..100] [--overwrite]
grainroom presets list
grainroom presets show ID
grainroom parameters list
grainroom probe
```

`gui` is the sole GUI-launching command. It derives `grainroom-gui` from the
directory of the running `grainroom` executable and passes an input path as the
two arguments `--input`, `PATH`. It never resolves the GUI through `PATH`.
Paths remain native `PathBuf`/`OsString` values, so non-UTF-8 Linux filenames
are not converted or logged.

`develop` resolves a built-in preset and validates all typed overrides as one
transaction. Duplicate `--set` parameter IDs are rejected. Output format is
inferred case-insensitively from `.jpg`, `.jpeg`, `.heic`, or `.heif` when
`--format` is absent. The command does not inspect the input or destination at
this stage. A valid request returns unavailable until the job and encoder
boundary is integrated; no destination is created.

Catalog, registry, and probe data is compact JSON on stdout. Probe JSON reports
only the backend name and availability, never executable paths. Diagnostics are
human-readable on stderr and avoid echoing input/output paths.

## Exit status

- `0`: success, help, or version;
- `2`: usage, range, unknown preset, duplicate override, or invalid format;
- `69`: a validated operation is currently unavailable, including a missing
  packaged GUI sibling or the not-yet-wired develop executor;
- `70`: internal failure.

When `gui` starts successfully, the child application's ordinary 0–255 exit
status is propagated.
