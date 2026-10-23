# Source registry: current rows, not replay history

The [implementation guide](../../../docs/implementation/README.md) defines the connected path.
This file owns source-row mutation. [Reconcile](../search-source-reconcile/FUNCTIONS.md) owns observation
meaning; [preparation](../../search-prep/search-materializer/FUNCTIONS.md) owns result reuse/retry/installation.
R011 revises the proposed conditional update; it does not implement it in Rust.

## Current code and why it is replaced

DirectRegistryOwner::prepare_output/prepare_source_output bind RegistryJournal through bind_original_v4,
restore all SourceRegistry state, load operations, reconstruct original policy/admission inputs, requalify
records, commit a generic batch, read changed targets back and rebuild the full control snapshot. The normal
native-source command also drops its acquired body and reports no retained revision.

The registry_control support files previously measured over 63 KiB before the main adapter. This is not a
small current-head transaction. Keep useful current values and database operations, not the replay protocol.

| Existing area | Actual disposition |
|---|---|
| Source-v2 preparation/commit/recovery | Connected to daemon admission but stores original-input metadata, not acquired bytes plus a usable RevisionRef. Replace the caller path. |
| Generic root/source/membership operations | In-memory registry, control ports, operation IDs/digests and receipts. Replace with concrete current-row transactions. |
| registry_snapshot and view helpers | Clone roots, sources, memberships, portfolios/history, generations and namespace state. Query uses PublishedView instead. |
| Reference portfolio/history, namespace cutover | No connected local query/source-writer consumer was found in the earlier audit. Do not build them to preserve historical APIs. |
| batch/restore and revision registration | Old daemon replay or test paths, not a connected retained-body head update. Remove with their consumers. |
| records/history forwarding crates | Relocate codecs/currentness/restore, but own neither bytes nor a database. Move remaining useful types and delete empty boundaries. |

## Current state and conditional application

RootRecord and SourceRecord hold current registration/selection, locator, availability and retained revision.
A locator-to-source index exists only where actual lookup needs it. Optional saved selections are current
configuration, not membership history. Read a command's affected rows directly; iterate only for inventory,
reconciliation or an actual publication/build snapshot.

A head digest is not a concurrency guard. A -> B -> A can restore the same content while an old acquisition
or preparation is still running. A membership Boolean can likewise change true -> false -> true after removal
and re-registration. The earlier compare/set wording did not distinguish these from unchanged state.

Capture the current source row revision and actual root-registration/settings state before the work. In the
short apply transaction, compare that guard plus the expected retained reference and selection conditions;
advance the current row revision when installing a source-state change. Recreated registrations must not reuse
a stale guard: use a new source/registration identity or a continuing non-reset row revision. No cryptographic
stamp, universal epoch service, operation archive or compatibility layer is needed.

Use this guard for source acquisition and prepared-result installation. Relevant locator, parser-selection
or policy changes cannot be ignored merely because content bytes match. Conversely, a concurrency revision
is not a transform-cache/point identity: unchanged pure results can be re-evaluated under current state without
pretending that the old task still owns the row. Never attach an artifact bearing another source's identity
just because its body digest matches.

## Connected source operation

```text
capture current registration/source/settings
-> pre-read admission
-> stable final-handle read
-> post-read classification
-> shared immutable object write/verification
-> short typed transaction:
     compare captured current-state guard
     set retained head, locator/physical observation, availability/selection
     advance source row revision
-> wake preparation/publication (optional acceleration)
```

Object I/O precedes the transaction. A failure/conflict can leave an unreferenced object for stopped-root
maintenance; it cannot overwrite a newer head. A known abort is failure. Resolve an uncertain commit by
point-reading current rows: an exact resulting state may establish the current outcome, while a newer state
is superseding work, not permission to retry old bytes. Do not rescan live paths under an old observation or
recover a completed-operation transcript merely to reconstruct history.

Prepare work remains discoverable from desired input versus current outcome after a lost wake-up. Published-only
sources remain discoverable from the publication inventory after removal/exclusion, even if the source row no
longer exists. An enqueue is not the durable record for either case.

## Operations the application actually needs

Register/update/unbind a root; get a root/source; bounded source/root enumeration; conditionally commit or
retire a current head without deleting retained objects; read one consistent source snapshot when needed;
resolve non-widening source selection against PublishedView. These responsibilities do not require public
methods for every noun, a generic repository trait or immutable corpus/portfolio history.

Observation refusal, incomplete inventory and confirmed removal follow the reconcile recipe. Same-path atomic
save need not create another logical source. A same-content rename reuses transformation only when the actual
preparation inputs/context also match; changed parser selection is not a metadata-only update.

## Published view versus source state

Current source rows are mutable desired input and can be newer than served published input. PublishedView
is created after preparation/Qdrant effects and identifies exact evidence. An old query stays coherent through
its view snapshot/guard, not through registry aliases/history. Do not call source-head commit search visibility.

Full-generation capture/restart belongs to [control](../../search-control-redb/FUNCTIONS.md#restartable-generation-input-capture),
not an in-memory snapshot label reopened against latest rows. Ordinary commands do not rebuild all registry state.

## Remove after cutover

Remove RegistryJournal/original-input resolvers/v4 restore; generic batch/operation replay; full SourceRegistry
reconstruction; query-time aggregate clones; unused reference-portfolio/history and namespace migration;
legacy DIRECT catalog/journal and process-local receipt/currentness authority. Delete forwarding packages once
their remaining values move. Unsupported old roots stay untouched; no universal migration is selected.

## Checks after code

Exact acquired body reaches the committed reference; stale acquisition cannot replace a newer head; same-content
ABA and remove/re-add reject old installation; current source row and locator change atomically; parser-changing
rename is reconsidered; incomplete inventory cannot mass-retire; lost preparation/removal wake-ups converge;
restart uses typed current rows without replay; old view evidence stays exact while heads advance. Run minimal
pinned compiler/Clippy for actual changed owners/consumers. These behaviors are not established by this document.
Preserve #598's actual unpublished object/key overlap, not its obsolete process programme.
