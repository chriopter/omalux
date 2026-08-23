# Color-management contract

Grainroom uses the safe `lcms2` Rust wrapper over the system Little CMS 2
library. The current build resolves `lcms2` through `pkg-config`; packaging
must provide the shared `lcms2` library and verify the final binary dependency.
No handwritten unsafe FFI or external profile asset is part of this module.

This is a codec-independent contract. It does not claim that a JPEG, PNG, HEIC,
or RAW decoder or encoder is currently connected to it.

## Profiles and working space

- Embedded ICC bytes are checked against `ResourceLimits::max_icc_bytes`
  before LCMS parses them. Empty, malformed, truncated, non-RGB (including
  CMYK), and transform-incompatible profiles fail explicitly.
- LCMS handles both matrix/TRC and general LUT RGB input profiles. The module
  does not inspect or approximate LUT contents itself.
- The generated working profile is scene-linear Rec.2020 with the BT.2020
  primaries `(0.708, 0.292)`, `(0.170, 0.797)`, `(0.131, 0.046)` and D65
  `(0.3127, 0.3290)`. Its TRCs are exactly linear. No external ICC file is
  loaded.
- The generated output profile is LCMS's standard sRGB profile. Its transfer
  function is not approximated by a simple power curve.
- Embedded-profile provenance stores the SHA-256 and byte length. An sRGB
  fallback is returned only by the explicit `assumed_srgb_profile` API and
  always carries `AssumedSrgb` provenance plus a warning diagnostic.

The numerical constants follow ITU-R BT.2020-2 for Rec.2020/D65 and IEC
61966-2-1 for sRGB. PNG `sRGB`, `gAMA`, and `cHRM` declaration behavior follows
the PNG Third Edition color-space precedence requirements, with contradictory
declarations rejected rather than silently choosing one.

## Transform behavior

Transforms use relative colorimetric intent and the fixed flags `NO_CACHE`,
`NO_OPTIMIZE`, and `COPY_ALPHA`. Grainroom owns no global mutable profile or
transform cache, and constructs each transform from explicit profile inputs.
`NO_OPTIMIZE` prevents LCMS from replacing the declared profile pipeline with
a machine-dependent optimized approximation.

Raster-to-working input is finite normalized straight-alpha RGBA in `[0, 1]`.
Negative or HDR raster samples are rejected before LCMS is called. Output is
finite, unbounded scene-linear Rec.2020. Straight alpha is copied from the
input after transformation so every alpha bit pattern, including positive
subnormals and signed zero, is preserved exactly.

Working-to-sRGB conversion requires an explicit `SdrRangePolicy`. `Reject`
fails transactionally if an output RGB channel is outside `[0, 1]`;
`ClipAndReport` clamps and counts clipped samples. There is no implicit tone
mapping.

Scanlines are length-checked and scratch allocation is preflighted through
`estimate_color_working_set`. Raster-to-working needs one RGBA-f32 scratch
buffer; working-to-raster needs input and output RGBA-f32 scratch buffers.
Both honor pixel-count, `u32` LCMS call-size, arithmetic-overflow, and working
memory limits before allocation.

## PNG declaration synthesis

The API does not parse PNG chunks. It accepts already decoded declarations for
future codec integration:

- `sRGB` alone, or accompanied by canonical `gAMA`/`cHRM`, generates sRGB.
- A non-sRGB declaration requires both `gAMA` and `cHRM`. The PNG image gamma
  is converted to the ICC decoding TRC exponent by `1 / gamma`.
- Missing pairs, non-finite/invalid gamma or chromaticities, degenerate
  primaries, and conflicts with `sRGB` are errors.

## Runtime provenance

The `lcms_version()` API records the linked runtime's encoded LCMS version.
Tests require the LCMS 2 major family and generate all ICC/LUT fixtures in
memory. They download no profiles and use no private image data.
