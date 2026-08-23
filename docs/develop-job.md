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

## Temporary resource-contract restriction

The existing pixel stages still contain legacy infallible allocations, and
there is not yet a complete stage-aware peak-memory estimator. Consequently
this job contract currently rejects every non-neutral resolved settings
document with `unproven_pipeline_budget`. It does not call the cloning pipeline
for a neutral document. This fail-closed restriction is removed only together
with a reviewed estimator and fallible allocation conversion for clarity,
effects, geometry and radial masks. RAW scene rendering is independently
bounded to the owned image plus two fallible scanline buffers.

`DevelopJobReport` has a versioned schema and deliberately excludes input and
output paths, filenames, service error strings and wall-clock timing. It keeps
only stable stage/error categories, content identity, signal relations and
deterministic processing counters.
