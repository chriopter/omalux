# Raster decode contract

`io::raster::decode_raster` is the bounded ingress path for JPEG, PNG, and BMP. It produces straight-alpha, linear Rec.2020 `f32` pixels with `SignalRelation::LinearizedDisplayReferred` and records the source bytes' domain-separated `SourceDigestV1`.

## Source identity and limits

- The path is opened once with `O_NOFOLLOW | O_CLOEXEC | O_NONBLOCK`. The descriptor must identify a regular file.
- The complete source is read once into a buffer under `max_source_bytes`. Signature sniffing, metadata scanning, digesting, and codec decode all consume that same immutable buffer. A rename cannot change identity and no decoder reopens the path.
- PNG dimensions are checked before compressed ICC metadata is expanded. All formats select an audited `RasterRgba8` or `RasterRgba16` working-set profile before pixel allocation.
- Cancellation is checked while reading source chunks, before and after codec work, and for every color-transform scanline. The third-party frame decode itself is synchronous and cannot be interrupted inside a codec call.

## Supported encodings

- JPEG: 8-bit grayscale or RGB. Four-component/CMYK and unsupported component layouts are rejected. A bounded, complete RGB ICC sequence is honored; otherwise the configured unprofiled policy applies.
- PNG: non-interlaced 8/16-bit grayscale, RGB, or RGBA. Palette, grayscale-alpha, `tRNS`, and APNG are intentionally rejected. Alpha remains straight and is normalized without color transformation.
- BMP: 8-bit RGB/RGBA output from the decoder, including padded and top-down rows. BMP has no accepted profile declaration, so the configured unprofiled policy applies.

PNG color declarations follow PNG Third Edition priority: `cICP`, `iCCP`, `sRGB`, then the complete `cHRM` + `gAMA` pair. The selected declaration is strict; contradictory lower-priority declarations are retained as provenance but do not override it. Duplicate selected declarations, unsupported HDR/YUV `cICP`, malformed ICC, and incomplete declarations are rejected.

## Metadata policy

Bounded TIFF IFD0 EXIF orientation values 1 through 8 are applied exactly once and the retained orientation tag is normalized to 1. EXIF containing a GPS IFD pointer is dropped as a whole and emits `MetadataDropped`; this avoids retaining opaque location subtrees. Invalid EXIF is also dropped rather than interpreted. XMP and IPTC are not imported by this initial decoder.

## Implementation provenance

The implementation is Grainroom code built on the public APIs of [`image` 0.25](https://docs.rs/image/0.25), [`png` 0.18](https://docs.rs/png/0.18), LCMS through the existing I2 color layer, [`crc32fast`](https://docs.rs/crc32fast) for chunk integrity, and [`flate2`](https://docs.rs/flate2) for bounded `iCCP` expansion. Format decisions follow the [PNG Third Edition specification](https://www.w3.org/TR/png-3/) and [CIPA Exif 3.0 specification](https://www.cipa.jp/std/documents/e/DC-008-Translation-2023-E.pdf). No codec implementation was copied into Grainroom.
