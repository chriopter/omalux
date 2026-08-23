# Preset catalog and command-line overrides

The built-in catalog is a deterministic, ID-sorted collection of validated
`PresetDocument` values. It includes `neutral`, the exact canonical
`DevelopSettings::default()` document, plus independently recreated parameter
documents maintained for Grainroom. These documents describe only Grainroom's
own public schema and names; they make no claim of upstream provenance or
endorsement. New built-ins require an explicit, reviewed recipe and must be
checked in as canonical schema-v3 JSON under `presets/builtin/`.

Schema v2 adds neutral-default `basics.exposure_ev` and the extended tone-curve
domain. Schema v3 adds `radial_masks.masks[].adjustments.exposure_ev`. Valid
schema-v1 documents migrate with neutral global and local Exposure EV; valid
schema-v2 documents retain global Exposure EV and gain neutral local Exposure
EV. V1 documents that attempt to use v2 fields or extended nodes, and v1/v2
documents that attempt to use the v3 local field, are rejected rather than
interpreted silently. Unknown fields remain denied at every level.

Built-ins use exactly compact canonical JSON followed by one LF; leading,
trailing, or additional whitespace is rejected during catalog construction.

External preset JSON is opened once on Linux with `O_NOFOLLOW | O_NONBLOCK`,
must be a regular file, and is bounded to 1 MiB before and during reading. Schema,
identity, unknown-field and setting validation remains owned by
`PresetDocument::from_json`. Catalog errors do not include source paths.

## Scalar and toggle overrides

`parameter.id=value` expressions are resolved against `parameter_registry()`:

- scalar values must be finite and inside the registry range;
- toggles accept exactly `true` or `false`;
- duplicate IDs are rejected;
- all changes are made to a clone, validated together, canonicalized and
  validated again;
- crop component overrides materialize a full-image crop and therefore allow
  `x` plus `width` (or `y` plus `height`) to be supplied in either order;
- quarter turns additionally require an integer.

Tone curves, crop presence, radial mask collections, mask identifiers and all
per-mask fields are structured data. They are rejected by the override parser
and may only be supplied through a complete validated preset JSON document.
This avoids ambiguous array addressing and order-dependent partial masks.

The exhaustive registry test requires every scalar/toggle ID either to have a
typed global-field mapping or to receive the explicit structured-data error.
Adding a parameter to the registry without updating this contract fails tests.
