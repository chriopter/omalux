# Preset catalog and command-line overrides

The built-in catalog is a deterministic, ID-sorted collection of validated
`PresetDocument` values. Grainroom currently ships only `neutral`: the exact
canonical `DevelopSettings::default()` document. There is deliberately no
invented “default look” or reference preset. New built-ins require an explicit,
reviewed recipe and must be checked in as canonical schema-v1 JSON under
`presets/builtin/`.

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
