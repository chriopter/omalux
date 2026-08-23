# Develop job boundary

`grainroom::job` is the Qt-free orchestration contract shared by a future CLI
and GUI adapter. Its fixed order is:

1. validate options and cancellation;
2. decode once;
3. resolve a validated catalog/document preset and typed scalar/toggle overrides;
4. derive `DevelopRenderContext` from the decoder's content digest and preflight
   the canonical `DevelopPipeline`;
5. render only `SceneRelatedRaw` through the versioned scene-to-display policy;
6. hand only `WorkingArtifact<DisplayReferred>` to an encoder.

The relation is a Rust type parameter. Scene rendering consumes a
`WorkingArtifact<SceneRelated>` and creates a distinct display artifact. It
does not relabel a RAW `DecodedPhoto`, whose provenance correctly remains
scene-related.

## Security and integration boundary

The current service trait accepts a path because no unified raster/RAW decoder
service has been integrated yet. `PhotoDecoder::decode_path_once` requires its
production implementation to use one safely opened regular file for sniffing,
digesting and decoding. The type system cannot prove that a path implementation
does this; codec integration must retain the existing single-open, `NOFOLLOW`,
bounded-source tests. It must also return the `SourceFileIdentity` captured from
that same open file. Publication compares this identity with the destination
and never re-resolves the input path. Fake services test orchestration only.

The production encoder receives a `PublicationRequest` containing this source
identity and the explicit `Forbid`/`Replace` policy. It must use atomic output.
Returning from the encoder is the publication commit point: later cancellation
cannot turn a visible file into a retryable error. `PublishedButNotDurable` is a
separate successful-but-degraded report outcome and must not be retried blindly.

Cancellation is cooperative. The runner checks between all stages and passes
the same token into codecs. A codec must poll it during long reads, conversion
and encoding. The current pixel pipeline is transactional but not internally
interruptible, so cancellation requested during a single pipeline call takes
effect immediately after that call.

## Bounded composable develop execution

The job resolves catalog/document settings and typed overrides before selecting
an explicit component profile. PointwiseV1 executes Exposure EV, brightness, contrast,
highlights, shadows, whites, blacks, saturation, vibrance, temperature, tint,
fade, vignette, and deterministic content-digest grain. ColorV1 additionally
executes structured tone curves from preset JSON plus scalar color mixer and
grading settings/overrides. SpatialV1 additionally admits global clarity,
bloom, halation, and sharpness with a conservative full-plane/tile peak.
Color and spatial families use the larger sequential scratch peak. GeometryV1
adds fallible orthogonal/projective/crop images. RadialMasksV1 adds one exact
largest-ROI output while analytic coverage and its local-sharpness kernel stay
on the stack. Local Exposure EV shares the global scene-linear exposure kernel
and adds no scratch. Negative local sharpness remains fail-closed before mutation.

The develop peak is the resident source image plus its exact transactional
copy. For RAW, scene-to-display follows after that copy has committed and been
dropped; its peak is the resident post-develop image plus two scanlines. The
job reports and enforces the maximum of those sequential phases, never their
sum. A limit failure is reported before develop mutation or encoder dispatch.

`DevelopJobReport` has a versioned schema and deliberately excludes input and
output paths, filenames, service error strings and wall-clock timing. It keeps
only stable stage/error categories, content identity, signal relations,
bounded profile/byte estimates, deterministic processing counters, output
format, and path-free codec provenance. The explicit component profile advances
the report schema to version 4.
