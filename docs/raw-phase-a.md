# RAW Phase A

This phase exposes a bounded library decoder contract. It is not yet wired to
the GUI loader or CLI/export job path; those integrations must call this API
explicitly and preserve the checked `DecodedPhoto` color/signal invariants.

Grainroom stages the source once while computing its content digest, then gives
that private immutable copy to the installed LibRaw `dcraw_emu`. The fixed
production arguments request camera white balance when available, embedded
matrix use, highlight clipping, AHD demosaic, linear 16-bit full resolution,
Rec.2020, and a private PPM output file:
`-w +M -H 0 -q 3 -4 -o 8 -Z <private-output> <private-input>`.

On Linux, the original is opened once with `O_NONBLOCK|O_NOFOLLOW`, verified as
a regular file, and copied through that descriptor. A CSPRNG-named `0700`
session directory and its `0600` files are created exclusively with operations
relative to held directory descriptors. Their exact modes are re-applied and
verified after creation, including under a restrictive process umask. The
decoder receives only
`/proc/self/fd/<dirfd>/<basename>` paths through an inherited directory
descriptor. Renaming or replacing the staging parent therefore cannot redirect
the decoder. FIFOs, sockets, devices, and source symlinks are rejected before a
blocking read. All staging objects have RAII cleanup on every return path.

The decoder is launched without a shell through the absolute `/usr/bin/prlimit`
and fails closed when that limiter is unavailable. Address-space, data, output
file-size, and CPU limits derive from the audited `ResourceLimits` and timeout.
Grainroom still treats the external LibRaw process as untrusted: the process
group is monitored until both its leader exit is observed without reaping and
the bounded stderr pipe reaches EOF; cancellation, timeout, capture failure, or overflow kills the
whole group. Even apparent success is followed by a process-group existence
check while `waitid(..., WNOWAIT)` deliberately leaves the exited leader
unreaped. That zombie pins the numeric PID/PGID, excluding ID reuse and
collateral group signalling. Any descendant that closed the capture pipe is
terminated during a bounded grace period, then the pinned leader is reaped
exactly once. A successful output is
stream-parsed: its bounded PPM header and
dimensions are validated before pixel allocation, the exact payload is read,
and trailing data is rejected. Cancellation is polled per scanline and within
wide scanlines so partial conversion is dropped with the private staging tree.
OS resource limits reduce risk but are not a general syscall sandbox.

Orientation is intentionally left to LibRaw: neither `-t` nor `-j` is passed.
The resulting metadata marks orientation consumed. `dcraw_emu` on the target
system has no supported version flag, so provenance records an unknown backend
version and emits a diagnostic. Camera-white-balance fallback also remains
explicitly unknown. Metadata loss, the decoder's normalized/clipped 16-bit
output range, and unknown matrix identity are always represented by diagnostics
and typed provenance flags. Phase A does not claim ICC colorimetric accuracy.

The repository intentionally contains no fabricated camera RAW fixture. The
real-backend probe therefore reports `Available` with an absolute executable
or `Unavailable`; it never treats an untested decoder as a successful decode.
