# Preparation: one product, recoverable current work

This file owns the retained-source -> current-preparation handoff. It is a target recipe, not a claim that
new preparation is connected. The source owner commits retained heads; publication owns external effects.

## Useful source and costs

| Source at c8e38b6 | Keep / change |
|---|---|
| `product/read.rs`: `open_exact_revision` | Read the exact retained object, not its mutable pathname. |
| `decode/*`, `normalize/*`, `maps/*` | Keep decoding, coordinates and explicit loss; remove avoidable copies. |
| `product/pipeline.rs`: `materialize_text_or_code` | One private materialization product. |
| `markdown/pipeline.rs`, `jats/pipeline.rs` | Keep parser/structure extraction and pass the same product onward. |
| `verify_markdown`, `verify_jats` | Do not reopen/reparse between adjacent in-process stages; validate independently loaded artifacts at their boundary. |
| `legacy_artifact/context.rs` | Duplicate object writer; remove after the shared-store cutover. |
| daemon `preparation_composition.rs` | Already distinguishes stored unsupported-input gaps from searchable layouts, but still preserves legacy output and receipt fields. Keep the distinction, not those fields. |

Valid UTF-8 need not be copied twice. UTF-16 conversion need not retain several whole-input scalar arrays.
Encode prepared text/maps/structure once at persistence, using the shared object writer. Materializer owns
no stage filenames, encryption lifecycle or completed-operation receipts.

## Desired state survives loss of the queue

A source-head commit followed by an in-memory enqueue has a crash window. A queue is a wake-up mechanism,
not the only evidence that work is needed.

The source owner commits the current retained revision and the input facts needed to derive preparation in
one short transaction. A preparation result records the exact retained revision, preparation profile and
all context actually used by that transform. These are ordinary current rows, not a durable job history.

Startup, explicit sync, queue overflow and a bounded background reconciliation walk compare current source
inputs with current preparation outcomes. Missing or mismatched outcomes mean work remains. A lost wake-up
must converge without another file edit. Iterate bounded source-key pages; do not scan the corpus per query
or add another scheduler/lease/operation database. The same comparison must run when a relevant profile or
context changes without a source-byte edit.

Retain only the current outcome per wanted input: a usable artifact (possibly with gaps), an explicit
unsupported/failure reason, and finite retry timing where needed. Unsupported unchanged input is not retried
on every scan. Transient failure has bounded attempts/backoff and remains visible; changed input/profile or
explicit retry may make it actionable again. Cancellation never becomes successful preparation.

## Worker completion cannot overwrite a newer source

1. Capture an exact wanted input from committed state, including current membership/availability and all
   preparation-affecting profile/context fields. Work on its immutable object outside the redb writer.
2. Materialize once, unitize the same product, encode and persist through the one object writer.
3. In a short transaction, compare the captured wanted input with the source's actual current wanted input
   and membership. Install the prepared reference/outcome only if they still agree.
4. If the source changed, was removed, or its relevant profile changed, do not overwrite current state with
   the stale completion. The artifact may be reusable by its exact input or become unreferenced; reconcile
   the actual current source again.
5. Publication compares eligible current prepared inputs with its published source inventory. A dropped
   publication wake-up is likewise reconstructible. Source changes during a build remain differences after
   that build, not only entries in a process-local queue.

Comparing a content digest alone is insufficient when context/profile/membership differs. Conversely, an
A -> B -> A edit may reuse the same pure transform if its complete input matches; that reuse does not revive
an old occurrence birth, closed Qdrant interval or stale published view. Those decisions belong to the
[unitizer](../search-unitizer/FUNCTIONS.md) and [publisher](../../search-index-qdrant/search-publication/FUNCTIONS.md).

Before a pending external publication is durable, a stale candidate can be replanned. Once effects may
have occurred, finish/recover its fixed operation; do not substitute a newer preparation or fresh births.
The newer desired state remains the following update. This is not permission to expose a mixed view.

## Successful empty is not failed extraction

| Input/outcome | Required meaning |
|---|---|
| A fully read supported empty text file | Retained zero-length revision; successful preparation with zero units. |
| Supported complete transform yielding zero searchable units | Explicit successful empty result for that transform, with source binding preserved. |
| Unsupported input, failed parser, truncated read, cancelled work, or scanned page without a selected extractor | An explicit gap/incomplete result, never successful empty or proof of no content. |
| Supported mapped/normalized text | Usable under its declared transform with coordinate/loss information; not automatically rejected merely because bytes changed. |

Do not fabricate a sentinel unit/vector or reuse an old nonempty artifact to make an empty source pass.
At c8e38b6, projection `validate_scope_binding` and `validate_projection_manifest_metadata` reject empty
units/manifests. Change those consumers with preparation. A nonempty -> empty update closes old points and
commits the new source outcome; an initially empty source needs no upsert. Removing the last source may
publish a valid empty inventory. Empty does not mean missing generation, unavailable Qdrant or failed build.

A lossy/partial representation may supply supported excerpts while reporting its gaps. It cannot establish
raw-byte completeness or a complete-negative exact result for content it did not represent.

## Donor use and evidence after code

Use ownership transfer with `String::from_utf8`, pulldown-cmark source offsets and quick-xml borrowed events.
Reuse selected structural split/merge mechanisms, not another parser/cache/storage framework.

Check one read/parse, mapped ranges and loss, crash after head commit but before enqueue, lost wake-ups,
profile-only change, B finishing before A, source removal during work, permanent unsupported input without
busy retry, zero-length input, nonempty -> empty and pending publication followed by a newer head. Verify
bounded memory and exact restart input. Run minimal compiler/Clippy after implementation; these scenarios
are not claimed executed by this document. Coordinate actual #598 object/key overlap only.
