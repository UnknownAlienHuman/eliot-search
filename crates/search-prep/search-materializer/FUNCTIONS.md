# Preparation: one product, recoverable current work

This owner turns a retained source into one current preparation outcome. [Unitizer](../search-unitizer/FUNCTIONS.md)
owns ranges; [objects](../../search-source/search-revision-store/FUNCTIONS.md) own file I/O; publication owns index effects.
This is a target recipe, not an executed-product claim.

## First connected format

Stage 2 starts with explicitly selected UTF-8 text/code and no normalization. Verify the retained object, move
valid UTF-8 into one owned string, and give that product to line-preferred unitization. Supported empty input is
valid. No syntax tree, per-character identity map, model/tokenizer, format registry or optional worker is required.
Exact UTF-8 byte ranges already describe unchanged text; create maps only for actual coordinate-changing transforms.

Other decodings or Markdown/XML/document structures join with their real evidence consumer. Preserve their
coordinate/loss semantics and maintained parsers; a decoder alone does not establish end-to-end support.
Unsupported input reports its real cause, never success with an empty body or a different implicit format.

```text
capture source/input/settings guard
→ check execution limits → verified retained read → selected transform
→ unitize the same owned product → encode once through shared object writer
→ short conditional current-outcome commit → wake publication
```

Use ExecutionContext: original deadline, cancellation and actual input/output/work limits. No PublishedView,
query scope or BM25 settings exist here. Keep the result private to the actual preparation consumers, not a
Raw/Parsed/Plan/Permit/Receipt family.

## Separate reuse, installation and retry decisions

| Decision | Compare | Consequence |
|---|---|---|
| Reuse complete output | Exact retained input plus output-affecting parser/version, decoding, normalization, unitization and path/language/context | Reuse only with correct source binding; equal bytes in another source do not authorize another source's artifact. |
| Install outcome | Captured non-reset source/registration/settings guard and expected current outcome | Apply in one short transaction; stale completion cannot overwrite a newer head or downgrade usable output. |
| Retry a gap | Actual failed condition and bounded retry timing | A relaxed limit or newly available selected extractor may permit retry without a source edit; unchanged permanent failures do not busy-loop. |

The [source-row owner](../../search-source/search-source-registry/FUNCTIONS.md#current-state-and-conditional-application)
defines the compare/set guard, including A→B→A and remove/re-add. It is not transform identity. A new operation may
reuse still-correct pure output after checking its new binding; an old task does not regain ownership.

Fail-only ceilings, scheduling policy, tests and review labels do not change successful transformed bytes.
Changing chunking/normalization/context does. Lowering admission limits may change eligibility/publication without
changing the identity of existing bytes. Partial output keeps its gaps; increasing a limit cannot relabel it complete.
Cancellation is not permanent unsupported input. Keep one current outcome, not an attempt archive or job service.

## Enforce the execution limit before acquisition

Check cancellation/deadline, supported format and declared length against the actual execution limit before read,
allocation or hash. Carry those limits into the concrete reader; a post-return Vec length check is too late.
Check actual bytes during reads and decoded text/maps/events/units before retaining them. Limits still apply to
empty input and cached outcomes where relevant; no second budget supplied only to a validation wrapper.

Move owned UTF-8 without a duplicate normalization buffer. For a selected UTF-16 transform, build necessary
coordinates while decoding instead of retaining several per-scalar arrays. Keep one representation buffer plus
unit ranges as specified by the unitizer. Serialize through a capped sink once; do not encode a full tree merely
to discard it after a size check. Object integrity and blocked-I/O limitations remain with the real reader.

## Prepared artifact acceptance

An in-process product goes directly from its producer to unitization; do not reopen the source or replay a parser
at that trusted handoff. Loading an app-produced saved artifact is a different boundary: check its exact ObjectRef
and integrity, supported format, retained-input/transform binding, finite decoded collections and valid ranges/maps.
A valid saved artifact is consumed directly, not verified by reconstructing an expected parse of the entire source.
This is reuse of known derived data, not acceptance of caller-supplied provenance claims or arbitrary external formats.

Missing, incompatible or corrupt derived data is unavailable. Preparation may explicitly rebuild it from the exact
retained input; Search/Continue/Expand do not secretly reprepare or substitute live-path bytes. Regeneration produces
a new verified outcome under the ordinary guard. A failed rebuild preserves the actual error.

## Desired state survives queue loss

The current outcome names its exact input/settings. Stage 3A startup, explicit sync and bounded reconciliation
compare desired source state with those outcomes, including configuration-only changes. A queue is acceleration,
not the only record of work. Coalesce per-source work in the existing scheduler; no new persistent task framework.

Read/transform/unitize/write outside a redb writer, then compare/install. Late failure cannot replace a usable
result for the same wanted input. Stale work may leave an unreferenced object, never revive a removed source.
[Publication](../../search-index-qdrant/search-publication/FUNCTIONS.md#reconcile-desired-and-published-state)
compares desired and published inventories, including removed/gap sources. An already dispatched pending build
keeps its fixed input; newer desired state is later work, not a replacement for bytes in its pending pages.

## Successful empty is not failed extraction

| Outcome | Meaning |
|---|---|
| Fully read supported empty text or complete transform with zero units | Successful empty with source binding; no sentinel unit/vector. |
| Unsupported, failed, truncated or cancelled work | Explicit gap/incomplete outcome, not absence of content. |
| Supported transformed/partial text | Actual representation, mapped coordinates and coverage/gaps; not raw-byte completeness. |

Remove early empty-input refusals and downstream nonempty assumptions together. Nonempty→empty changes desired
membership through publication; it does not trigger preparation-owned point deletion. A missing collection or
failed initial build is not an empty valid view.

<a id="inspected-source-useful-work-and-actual-defects"></a>
## Source seams

The [pinned earlier audit](https://github.com/UnknownAlienHuman/eliot-search/blob/d83e1e27d27768b9803b5fd809c1631ff4cf73d7/crates/search-prep/search-materializer/FUNCTIONS.md#inspected-source-useful-work-and-actual-defects)
records request/context budget disagreement, empty refusals, all-fields profile hashing and duplicate writers.
At implementation, trace product/read.rs, request/validate.rs, product/pipeline.rs, profile/digest.rs and their
actual daemon/unitizer consumers. Do not fix a dormant wrapper while the real caller keeps the old behavior.

## Markdown donor correction

The [donor register](../../../docs/audit/DONOR_VERIFICATION_REGISTER_2026-10-09.md#markdown-offset-iterator-released-fix-not-yet-adopted)
separates main's earlier 0.13.1 pin from the unmerged branch's 0.13.5 correction and compiler evidence. Recheck
current manifests at the Markdown code cut; adopt the exact dependency/lock/transform-identity change together.
Keep OffsetIter and declared options, full event/range semantics and the existing raw control-input policy.
Do not fake offsets, blacklist a reproducer, add a catch wrapper or block unrelated plain UTF-8 work on Markdown.
The [detailed prior cases](https://github.com/UnknownAlienHuman/eliot-search/blob/d83e1e27d27768b9803b5fd809c1631ff4cf73d7/crates/search-prep/search-materializer/FUNCTIONS.md#markdown-donor-correction)
retain the printable reference-definition/list regression, balanced-event, UTF-8-range and real consumer checks.
Those are historical investigation/remaining cases, not a fresh upstream audit or current-main test result.

## Removal and checks after implementation

Move real consumers and remove adjacent replay, duplicate writers, legacy DIRECT/receipt/profile formats and
exclusive fixtures. No compatibility parser, crypto integration, new cache service or job ledger is selected.
Preserve user data/private work. After code run minimal locked check/Clippy for changed owners and immediate consumers.

Later cases: unchanged UTF-8/empty, actual transformed coordinates, pre-read versus execution limits, one read/parse,
saved-artifact reuse without reparse, corrupt artifact and explicit rebuild, output bounds, settings-only retry,
lost wake-up, stale success/failure, ABA/remove-add and parser-changing rename. No such execution is claimed here.
