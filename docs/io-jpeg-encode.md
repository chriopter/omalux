# JPEG encode boundary

Omalux's first production encoder accepts only finite, linear Rec.2020/D65
pixels whose `SignalRelation` is `LinearizedDisplayReferred`. Scene-related RAW
is rejected before allocation or destination creation; it must first pass the
separate scene-to-display renderer. The only current output profile is sRGB.

## Deterministic preparation

The existing LCMS `WorkingToSrgbTransform` converts one bounded scanline at a
time. SDR range handling is explicit: reject transactionally, or clip and count
each affected RGB sample. Straight alpha is either rejected or composited over
an explicit linear-Rec.2020 background before color conversion. Encoded sRGB is
quantized with `floor(sample * 255 + 0.5)`. The default JPEG quality is 90.

Dimensions, pixel count and the resident image are gated before metadata is
inspected. EXIF then uses an allocation-free first pass over a fixed-size
allowlist to compute the exact rebuilt TIFF size. Only after the full peak is
accepted are EXIF, generated profiles and RGB output allocated; Omalux-owned
buffers use checked arithmetic and fallible reservations.

The named `JpegRgb8` working-set profile accounts for the resident image,
input metadata, RGB preparation, scanline transform scratch, canonical sRGB
and Rec.2020 profiles, and the audited `image` 0.25.10 encoder transients. The
latter include encoder-owned ICC/EXIF copies, the six-byte APP1 identifier, the
14-byte ICC chunk header, and its fixed tables/header buffer. The canonical
profile byte sizes are pinned by tests and a mismatch fails closed. Codec writes
pass through a counting writer and stop at `max_output_bytes`. The `image` JPEG compression call itself is
synchronous and cannot be interrupted internally; cancellation is checked
before and immediately after it, and a cancelled result is never published.

## Color and metadata

Every file embeds Omalux's canonical generated sRGB ICC profile. EXIF is
never copied opaquely. A bounded TIFF reader extracts a small numeric technical
allowlist and constructs a fresh TIFF tree. GPS, orientation, MakerNote,
UserComment, camera or lens serial numbers, free text, thumbnails and unknown
tags are omitted. XMP and IPTC are omitted in this phase because both can carry
free-form location or path data. Malformed EXIF is dropped and reported.

JPEG APP1 has a 65,533-byte payload limit. After the six-byte `Exif\0\0`
identifier, Omalux permits at most 65,527 TIFF bytes and validates this
before calling the codec.

## Publication and errors

Encoding uses `write_atomic_output`; overwrite remains forbidden by default.
Input/output inode collision protection is preserved. Codec, cancellation and
resource failures leave no destination or temporary name. Atomic publication
errors retain their original type, including `PublishedButNotDurable`, which
must never be retried blindly.

The implementation uses the public JPEG encoder API of `image` 0.25.10
(MIT OR Apache-2.0) and does not copy codec source. Complete dependency notices
are in `THIRD_PARTY_NOTICES.md`.
