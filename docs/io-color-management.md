# Color-management contract

Grainroom uses the safe `lcms2` Rust wrapper over the system Little CMS 2
library. The current build resolves `lcms2` through `pkg-config`; packaging
must provide the shared `lcms2` library and verify the final binary dependency.
No handwritten unsafe FFI or external profile asset is part of this module.

This is a codec-independent contract. It does not claim that a JPEG, PNG, HEIC,
or RAW decoder or encoder is currently connected to it.
The implemented LCMS input transform covers encoded RGB raster pixels only;
it is not a RAW demosaic or camera-color pipeline. Scene-related RAW working
pixels are intentionally rejected by the sRGB encoder until an explicit
scene-to-display rendering stage exists.

## Profiles and working space

- Embedded ICC bytes are checked against `ResourceLimits::max_icc_bytes`
  (4 MiB by default)
  before LCMS parses them. Empty, malformed, truncated, non-RGB (including
  CMYK), and transform-incompatible profiles fail explicitly.
- LCMS handles both matrix/TRC and general LUT RGB input profiles. The module
  does not inspect or approximate LUT contents itself.
- The generated working profile is linear Rec.2020 with the BT.2020
  primaries `(0.708, 0.292)`, `(0.170, 0.797)`, `(0.131, 0.046)` and D65
  `(0.3127, 0.3290)`. Its TRCs are exactly linear. No external ICC file is
  loaded.
- The generated output profile is LCMS's standard sRGB profile. Its transfer
  function is not approximated by a simple power curve.
- Embedded-profile provenance stores the SHA-256, byte length, and exact LCMS
  runtime version. An sRGB
  fallback is returned only by the explicit `assumed_srgb_profile` API and
  always carries `AssumedSrgb` provenance plus a warning diagnostic.
- Every generated ICC uses the fixed valid header creation time
  `2000-01-01T00:00:00` and an unset (all-zero) optional profile ID. Grainroom
  reopens and validates those exact canonical bytes, stores them in the
  `RgbProfile`, and hashes/exports the same bytes. This removes LCMS's wall-clock
  timestamp from generated sRGB, Rec.2020, cICP, and gAMA/cHRM provenance.

The numerical constants follow ITU-R BT.2020-2 for Rec.2020/D65 and IEC
61966-2-1 for sRGB. PNG behavior follows the exact priority in
[PNG Third Edition section 4.3, Table 1](https://www.w3.org/TR/png-3/#4Concepts.ColourSpaces).

## Transform behavior

Transforms use relative colorimetric intent and the fixed flags `NO_CACHE`,
`NO_OPTIMIZE`, and `COPY_ALPHA`. Grainroom owns no global mutable profile or
transform cache, and constructs each transform from explicit profile inputs.
`NO_OPTIMIZE` prevents LCMS from replacing the declared profile pipeline with
a machine-dependent optimized approximation.

Raster-to-working input is finite normalized straight-alpha RGBA in `[0, 1]`.
Negative or HDR raster samples are rejected before LCMS is called. Output is
finite, unbounded, **linearized display-referred** Rec.2020, never relabeled as
scene-referred. `ColorTransformReport::working_signal_relation` and PNG resolution
results carry `LinearizedDisplayReferred`; a future decoder must copy that into
`DecodedPhoto`. Straight alpha is copied from the input after transformation
so every alpha bit pattern, including positive subnormals and signed zero, is
preserved exactly.

Working-to-sRGB conversion requires both the input `SignalRelation` and an
explicit `SdrRangePolicy`. `SceneRelatedRaw` returns the typed
`SceneToDisplayRenderingRequired` error transactionally: a future tone/display
rendering contract must run first. `LinearizedDisplayReferred` is accepted. `Reject`
fails transactionally if an output RGB channel is outside `[0, 1]`;
`ClipAndReport` clamps and counts clipped samples. There is no implicit tone
mapping.

Scanlines are length-checked and scratch allocation is preflighted through
`estimate_color_working_set`. The estimate charges all serialized source,
working, and output profiles in addition to transform scratch.
Raster-to-working needs one RGBA-f32 scratch
buffer; working-to-raster needs input and output RGBA-f32 scratch buffers.
Both honor pixel-count, `u32` LCMS call-size, arithmetic-overflow, and working
memory limits before allocation.

LCMS also performs opaque native allocations while parsing profiles and
constructing transforms; the safe wrapper exposes no complete preflight for
those allocations. The 4 MiB input cap, known-small generated profile types,
post-serialization checks, and caller scratch accounting reduce but do not
eliminate that risk. A sandboxed codec/color worker with OS memory limits is a
required hardening step before hostile-file production use.

## PNG declaration synthesis

The API does not parse PNG chunks. It accepts already decoded raw chunk values
for future codec integration and resolves understood declarations in the
normative order `cICP > iCCP > sRGB > cHRM+gAMA`. Once a higher-priority chunk
is selected, all lower-priority chunks are ignored even when their values
would be contradictory or invalid. Duplicate or invalid selected chunks remain
errors.

- Selected `iCCP` bytes use the same bounded ICC opening path.
- Full-range cICP with BT.709 primaries and either BT.709 or sRGB transfer is
  supported. Non-RGB matrix coefficients are invalid. PQ (16) and HLG (18)
  produce a typed unsupported-HDR error until an HDR decode path exists;
  unsupported primaries/transfers and narrow range also fail explicitly.
  `PngCicpFields::try_from_raw` retains the raw bytes and rejects full-range
  flag values other than 0 or 1 before profile resolution.
- Selected `sRGB` validates only its rendering-intent byte and ignores lower
  compatibility chunks.
- Selected `gAMA+cHRM` retains the raw PNG integers. gAMA zero is invalid;
  every nonzero PNG integer produces a finite, normal reciprocal decoding
  exponent. Missing pairs and invalid/degenerate raw chromaticities fail.

`ColorProvenance::PngDeclared` records the selected source, raw cICP/sRGB/gAMA/
cHRM values, selected embedded-ICC provenance where applicable, and the digest,
serialized size, and LCMS version of the resolved/generated ICC profile.

## Runtime provenance

The `lcms_version()` API, ICC/PNG provenance, and every transform report record
the linked runtime's encoded LCMS version. Pixel output is deterministic only
for the same profiles, flags, architecture contract, and pinned LCMS runtime;
it is not promised bit-identical across LCMS releases. Every comparison
manifest must record the exact `lcms_version` and reject mismatched runs.

Packaging TODO: pin the supported LCMS package version and add a final-binary
linkage/version gate. Tests currently require the LCMS 2 major family and
generate all ICC/LUT fixtures in memory. They download no profiles and use no
private image data.

## Decoder integration invariant

Codecs must construct results through `DecodedPhoto::new` rather than assembling
fields. The constructor validates the CPU image, pixel and working-byte limits,
metadata limits, and known provenance/relation pairs. PNG, embedded ICC, declared
or assumed sRGB are display-referred; `RawMatrix` is `SceneRelatedRaw`. Fields are
private and read through accessors so a valid result cannot later be made
internally inconsistent.
