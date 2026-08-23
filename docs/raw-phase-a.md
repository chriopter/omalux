# RAW Phase A

Grainroom stages the source once while computing its content digest, then gives
that private immutable copy to the installed LibRaw `dcraw_emu`. The fixed
production arguments request camera white balance when available, embedded
matrix use, highlight clipping, AHD demosaic, linear 16-bit full resolution,
Rec.2020, and PPM on stdout: `-w +M -H 0 -q 3 -4 -o 8 -Z -`.

Orientation is intentionally left to LibRaw: neither `-t` nor `-j` is passed.
The resulting metadata marks orientation consumed. `dcraw_emu` on the target
system has no supported version flag, so provenance records an unknown backend
version and emits a diagnostic. Camera-white-balance fallback also remains
explicitly unknown. Phase A does not claim ICC colorimetric accuracy.

The repository intentionally contains no fabricated camera RAW fixture. The
real-backend probe therefore reports `Available` with an absolute executable
or `Unavailable`; it never treats an untested decoder as a successful decode.
