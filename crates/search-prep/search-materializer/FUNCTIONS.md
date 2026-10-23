# Preparation: one product, recoverable current work

This is the recipe for retained source -> current preparation, not a claim that the replacement is connected.
The source owner commits retained heads; publication owns external effects. Source rechecked at `7c7fd36c`
on 2026-10-23. Keep deterministic decoding, coordinates and explicit gaps from the old design, not its receipts.

## Inspected source: useful work and actual defects

| Source | Keep / change |
|---|---|
| `product/read.rs::open_exact_revision` | Reads the retained revision, verifies returned length/digest and moves its buffer. It still rejects empty input. The read port receives neither execution budget nor cancellation. |
| `request/validate.rs` and `product/{model,pipeline}.rs` | Request validation checks its supplied budget; execution accepts an independently supplied context. The pipeline checks nonzero limits/profile, then reads the body without rechecking its length against the execution input limit. A request admitted with an 8 MiB budget can therefore call the reader under a later 1 KiB budget. |
| `profile/digest.rs` | Hashes behavior, all six resource ceilings and `golden` into one profile ID. A test-only or ceiling-only change alters that ID. |
| `profile/change.rs::classify_profile_change` | A changed ID either requires repreparation/reprojection or rejects a non-increasing revision. It does not distinguish unchanged successful output from changed execution policy. |
| `decode/*`, `normalize/*`, `maps/*` | Keep supported conversion, source coordinates and loss; remove unnecessary whole-input copies. |
| `product/pipeline.rs`, `markdown/pipeline.rs`, `jats/pipeline.rs` | One private materialization product, handed directly to unitization. |
| `verify_markdown`, `verify_jats`, `legacy_artifact/context.rs` | Remove adjacent-stage reparse and duplicate object writer with callers; validate independently loaded artifacts at their actual boundary. |
| daemon `preparation_composition.rs` | Distinguishes stored unsupported-input gaps from searchable layouts. Keep that distinction, not legacy output/receipt compatibility. |

These are source findings, not executed failure reproductions. The public request/context combination above
is the concrete budget counterexample; it is not a claim that every current caller supplies different budgets.

## Separate reuse, installation and retry decisions

These are ordinary input/current-state fields, not three new services or public token types.

**Transform reuse** depends on the exact retained input and behavior actually used: parser/extractor version,
encoding/normalization, unitization options and relevant language/path/context. A rename from `.txt` to `.md`
can change parser selection even with identical bytes. Keep source-bound coordinates/provenance correct;
equal content in another source is not permission to reuse an artifact carrying the first source's identity.

**Installation** depends on the current source registration and row state. Capture the source row revision,
root registration and selected settings used to derive the work. Compare them in the final short transaction,
not just content digest or a membership Boolean. The [source owner](../../search-source/search-source-registry/FUNCTIONS.md)
owns that current-state guard. Remove/re-add and A -> B -> A must not make an old completion current again.
A newer reconciliation may reuse a still-correct pure result after checking the new binding; it does not revive
old occurrence births, closed point intervals or old evidence metadata.

**Retry eligibility** depends on the reason and conditions that prevented completion. Store one current outcome
with its input, relevant failed limit/provider condition and finite retry timing. A limit failure must become
actionable when that limit is relaxed, even without a source edit; an unavailable selected extractor can become
available. Explicit retry is also allowed. Do not busy-retry unchanged permanent failures or let cancellation
become permanent unsupported content. Keep bounded backoff/attempts for transient failure, not an attempt archive.

Test fixtures, review reports, arbitrary profile labels and scheduling ceilings do not define transformed bytes.
Increasing a fail-only work/size ceiling must not invalidate unchanged complete successes across the corpus.
Lowering an admission limit may change current eligibility and require a publication update; it does not change
the identity of already produced bytes. A setting that actually changes output (chunk size, normalization,
selected extraction, intentional truncation) does belong to transform identity; partial output retains its
actual coverage and never becomes complete merely because a later budget increased. Do not preserve the old
all-fields profile hash by adding a compatibility wrapper.

## Enforce the execution limit before acquisition

Before invoking the retained-object reader, check cancellation and the declared retained length against the
actual execution limit and supported profile. Reject an oversized request before body I/O, allocation or hash.
The concrete shared reader also caps bytes as it accumulates/decodes them and validates final length/integrity;
post-return `Vec::len` checks alone cannot bound what a reader already allocated. Pass the operation's real
cancellation/deadline through the concrete read path rather than adding another read-port framework.

For a plain stream, standard `Read::take` can cap bytes; its EOF at the limit is not proof that the source ended,
and it supplies no time bound or interruption of blocked I/O. Keep the declared-length/oversize check and actual
platform cancellation separate. See [std Read](https://doc.rust-lang.org/std/io/trait.Read.html#method.take).

Move valid owned UTF-8 into its string where possible. Avoid several whole-input scalar arrays for UTF-16.
Encode prepared text/maps/structure once using the shared object writer. This owner has no stage-file,
encryption lifecycle or completed-operation archive.

## Desired state survives queue loss

The source owner commits its retained revision and preparation-relevant current facts in one transaction.
The result records the exact input actually prepared. Startup, explicit sync, overflow and bounded background
reconciliation compare current desired input/outcome, including relevant configuration-only changes. Iterate
source-key pages outside queries. A queue only accelerates this comparison; it is not a durable job database.

Coalesce repeated work for a source in the existing scheduler. Materialize outside the redb writer, unitize
the same product and persist it through the shared writer. Then conditionally install against the captured
source/settings state and current outcome. A late failure cannot overwrite a usable result already committed
for that same wanted input. Stale work may leave reusable pure output or an unreferenced object, but cannot
revive a removed source or overwrite a newer result.

Publication uses its [two-sided reconciliation](../../search-index-qdrant/search-publication/FUNCTIONS.md#reconcile-desired-and-published-state),
including published-only sources and explicit gaps, not only newly successful artifacts. Once an external
pending operation may have effects, finish/recover its fixed inputs; newer desired state is follow-up work.
Do not splice a new revision into the pending cursor or describe the old published revision as current input.

## Successful empty is not failed extraction

| Input/outcome | Meaning |
|---|---|
| Fully read supported empty text | Retained zero-length revision; complete preparation with zero units. |
| Complete supported transform with no searchable units | Successful empty for that transform, with source binding retained. |
| Unsupported parser/format, failed or truncated read, cancellation, scanned page without an extractor | Explicit unavailable/incomplete outcome, never proof of no content. |
| Supported mapped/normalized or partial text | Useful excerpts with declared transformation, coordinates and gaps; not raw-byte completeness. |

Change both early empty refusals in `request/validate.rs` and `product/read.rs`, as well as the planner's
EmptyUnitSet/manifest checks, when implementing empty support. No sentinel unit/vector or stale nonempty
artifact. Nonempty -> empty closes former current points through publication; the last source can yield a
valid empty view. Missing metadata/collection and failed initial build are not empty success.

## Removal and checks after implementation

Keep current Markdown/XML donor mechanisms and inert content. Remove duplicate writers, adjacent reparse,
old DIRECT/profile/receipt formats and exclusive fixtures with actual consumers. No optional worker gate,
new cache service, accepted-profile token or completed-job ledger.

Check one read/parse and ranges/loss; different validation/execution budgets before I/O; golden-only and
fail-only budget changes without success invalidation; resource failure becoming retryable after configuration
change; lost wake-ups; stale success/failure; remove/re-add and A -> B -> A; parser-changing rename; supported
zero input and gap transitions. Publication owns deletion/profile-build tests. Minimal pinned compiler/Clippy
follows source changes. No such runtime result is claimed here. Protect only actual #598 object/key overlap.
