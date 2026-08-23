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
                  [--max-source-bytes BYTES] [--max-pixels COUNT]
                  [--max-working-bytes BYTES] [--max-output-bytes BYTES]
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

`develop` runs the production decoder, relation-typed develop job, and atomic
codec dispatcher. JPEG, PNG, BMP, and camera RAW inputs are signature-routed after
one `O_NOFOLLOW` source open; the digest, source identity, and decoded bytes all
come from that same descriptor. RAW additionally requires a trusted functional
`dcraw_emu` at `/usr/bin/dcraw_emu` or `/usr/local/bin/dcraw_emu`.

Output format is inferred case-insensitively from `.jpg`, `.jpeg`, `.heic`, or
`.heif` when `--format` is absent. The default build rejects HEIC with exit 69
before preset, input, or destination I/O. A build with `--features heic`
validates a 10-bit libheif/x265 backend and publishes real HEIC through the same
atomic boundary as JPEG. Options and resource limits are validated before an
external preset is opened. `--preset` and `--preset-file` are mutually
exclusive, duplicate `--set` IDs are rejected, and external preset JSON is read
through the bounded no-follow loader.

The current job resource proof admits the bounded PointwiseV1 profile:
pointwise Basics controls except Clarity, plus Fade, Vignette, and Grain. Supported
presets and `--set` overrides are applied before encoding. The final report is
schema version 3 and names the output format, codec provenance, profile, and
its exact estimated peak. Clarity,
geometry, curves, color operations, radial masks, Bloom, Halation, and
Sharpness remain fail-closed with `unproven_pipeline_budget` and exit 69 after
decode, before develop mutation or output creation.

The destination defaults to no-overwrite atomic publication. `--overwrite`
permits replacement of an existing regular file, while source/destination inode
collisions and symlinks remain rejected. The same-open source descriptor is
leased through the encoder commit point. JPEG and HEIC are sRGB with an embedded
profile; HEIC additionally writes NCLX 1/13/1/full. Quality defaults to 90. Safe
metadata policy, alpha handling, SDR clipping, and
the supplied resource limits are applied at the encoder boundary.

`--progress human` writes completed stage names to stderr. `--progress json`
writes one path-free JSON event per completed stage to stderr. The final compact
JSON report selected by `--json` is written only to stdout and contains a
content digest, signal relations, scene-render summary where applicable, and
publication outcome—never paths. Human mode writes a path-free summary.
SIGINT cooperatively cancels the active decoder/renderer/encoder and exits 130;
pre-commit cancellation leaves no destination.

`published_and_durable` means the destination and its directory were synced.
`published_but_not_durable` means the destination is already visible but the
directory sync failed; this remains a successful commit-point report and must
not be blindly retried.

Catalog, registry, and probe commands produce human-readable stdout by default
(`list` output is TSV) and compact JSON with `--json`. Probe considers only
fixed system/package paths owned by root and not group/world writable, then
runs a bounded `dcraw_emu` behavior handshake. It never searches `PATH`.
That ownership/mode check is the executable trust boundary; the handshake is
not a sandbox for arbitrary programs. Capture pipes are nonblocking and
bounded, the original process-group leader stays unreaped while same-group
survivors are killed, and even a detached process holding a pipe cannot delay
the probe beyond its deadline.
Probe and production decoding call the same fixed-candidate resolver and
bounded behavior handshake, so reported availability cannot name a backend the
production decoder would refuse. Probe JSON reports only the backend name and
functional availability, never executable paths. Diagnostics avoid echoing
input/output paths.

## Exit status

- `0`: success, help, or version;
- `1`: operational input, decode, encode, output, or destination failure;
- `2`: usage, range, unknown preset, duplicate override, or invalid format;
- `69`: unavailable codec/backend, missing packaged GUI sibling, or a develop
  request whose pipeline working-set proof is not yet complete;
- `70`: internal failure.
- `130`: cancelled by SIGINT before publication.

When `gui` starts successfully, the child application's ordinary 0–255 exit
status is propagated.
