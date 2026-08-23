# HEIC encoding

HEIC output is an optional Linux system-codec integration. Build the core with
`--features heic`; this enables `libheif-sys 5.3.1+1.23.1` and dynamically
links the installed libheif. Packaging must provide a compatible libheif
runtime and the x265 HEVC encoder. Without the feature, the public API returns
`HeicBackendNotBuilt` without opening an output.

The capability probe selects the encoder whose stable ID is exactly `x265`.
It performs fresh synthetic 3x2 encodes at 8 and 10 bits and reads each result
back. Success requires coded depth, raw ICC payload, and NCLX `1/13/1/full` to
survive. Merely finding an HEVC descriptor is not considered capability.

Production preparation is shared with JPEG: input must already be linearized
display-referred Rec.2020, scene-related RAW fails before allocation, LCMS
produces encoded sRGB, quality defaults to 90, alpha follows the explicit
reject/flatten policy, and only sanitized EXIF enters libheif. ICC and NCLX are
both written. The C boundary is confined to `encode/heic.rs`; context, encoder,
image, handle, and encoding options are RAII-owned.

libheif writes synchronously through a callback into AtomicOutput's private
file. Each append checks `max_output_bytes` before writing, so no complete
encoded-byte buffer exists. Writer, codec, cancellation, resource, publication,
and post-publication durability errors retain their distinct semantics.

The estimator charges resident image, prepared RGB8, metadata, and a
conservative 96 bytes/pixel for opaque libheif/x265 native state. libheif/x265
exposes neither a caller allocator nor a hard memory ceiling, so this is a
policy estimate, not an enforceable native heap limit. Process isolation or an
RLIMIT remains packaging hardening.

Cancellation is checked before preparation, per preparation row, before native
encode, in each writer callback, and after native encode. x265 has no cancel
callback during one encode call, so cancellation latency can include that call.
Atomic publication still prevents a partial destination.

HEVC distribution may implicate patents or royalties depending on jurisdiction
and distribution model. Copyright licenses do not grant patent rights.
Distributors must perform their own legal review; Grainroom bundles neither
x265 nor libheif.
