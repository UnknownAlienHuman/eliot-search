# Control persistence — functions and implementation

Use the [implementation guide](../../docs/implementation/README.md#3-keep-control-state-small-and-publication-real).
This is a map of existing functions and their replacement responsibilities, not a frozen schema or
signature contract. Change the producer and immediate consumers together.

## Existing entrypoints

| Current entrypoint | Source | Treatment |
|---|---|---|
| `PersistentControlJournal::{create,open}` | [persistent.rs](src/persistent.rs) | Keep one actual disk owner. Validate the selected format and settings; refuse unsupported roots before writes. |
| `transact`, `transact_conditionally` | [persistent.rs](src/persistent.rs), [calls.rs](src/persistent/calls.rs), [transaction.rs](src/persistent/transaction.rs) | Replace generic operation/envelope machinery with short typed updates to current records. Check needed conditions inside the write transaction. |
| `commit_registry_batch`, `read_registry_snapshot` | [registry/transaction.rs](src/persistent/registry/transaction.rs) | Preserve atomic source/head metadata and coherent startup reads. Source identity, admission and retained bytes remain with the source owner. |
| `lookup_registry_operation`, `read_registry_operation_targets` | [registry/lookup.rs](src/persistent/registry/lookup.rs) | Retain targeted reads needed by actual callers; remove permanent operation-envelope lookup when its caller is replaced. |
| `persist_publication_intent`, `load_unresolved_publication` | [publication.rs](src/persistent/publication.rs) | Keep a bounded pending record for real unfinished external publication. |
| `commit_visible_epoch`, `read_publication_visibility` | [publication/visibility.rs](src/persistent/publication/visibility.rs) | Commit the published view after actual Qdrant effects/readback. A caller-supplied digest or receipt cannot substitute for those effects. |
| `control_snapshot_with_context`, `publish_committed_snapshot_with_context`, `recover_snapshot_publication_with_context` | [snapshot.rs](src/persistent/snapshot.rs) | Read committed state and publish one coherent in-memory view. Recover that view without replaying a completed mutation. |
| `recover_transaction`, `recover_registry_batch`, `recover_visible_epoch_commit` | [persistent.rs](src/persistent.rs), [registry/transaction.rs](src/persistent/registry/transaction.rs), [visibility.rs](src/persistent/publication/visibility.rs) | Resolve real uncertain state; simplify with the journal being replaced rather than preserving every historical receipt API. |

These functions already exist; their simplified forms are not implemented by this documentation.
The in-memory `ControlJournal` in [reference.rs](src/reference.rs) remains a reference model, not a
production fallback. The pure `publication_codec::cas` computes a new floor; it does not commit redb.

## Current records and transaction boundaries

The source owner supplies source/revision identity, locator, availability and head changes. This package
stores those records, explicit settings, artifact references and publication metadata. It does not
store source bodies, extracted text, postings, vectors or ranked-query history.

A useful mutation is direct:

1. Validate the typed command and finite input sizes.
2. Begin a redb write transaction and read the affected records/required generation conditions.
3. Apply the current-state change and commit.
4. Publish or refresh the corresponding in-memory view from committed state.

Keep the transaction short. No Qdrant call, source scan or preparation runs while holding its writer.
Normal lookups read the needed keys; a full inventory read is an explicit startup/recovery operation.
Read/query admission produces zero durable control writes.

Remove `ControlMutation` fingerprints, `ControlCommitReceipt` and opaque port/envelope plumbing where
their only remaining consumer is the retired replay archive. Keep a generation or operation identifier
only where it resolves an actual race or retry. Do not add another callback/work-meter hierarchy.

## Failure semantics to implement

| Observed outcome | Required behavior |
|---|---|
| Rejected before mutation | Return the actionable validation, cancellation or limit error. |
| Staged transaction aborted successfully before commit | Return the original error; that transaction did not commit. Do not retain an unknown-commit fence. |
| Commit result unresolved | Inspect current durable state before retrying the same effect. |
| redb committed, in-memory publication failed | Rebuild/publish the committed snapshot; do not rerun the mutation. |
| Qdrant write may have happened | Keep pending publication and inspect actual Qdrant/control state. Do not infer rollback from a timeout or a later redb abort. |
| Unsupported old format | Refuse before writes and leave that root untouched. |

The current `execute_scoped` marks cancelled/budget-exhausted staging unknown even when abort succeeds;
change this branch. Real commit uncertainty and external publication recovery remain distinct.
Use the request's actual deadline and concrete size/work limits without renewing the deadline at
each nested operation.

## Retire historical requirements

Schema-5 operating-profile/genesis/adoption records, schema-1–4 support and frozen class/table bytes
are existing old-format details. No new implementation must preserve them, support every old version
or perform a Direct → Indexed → migration-adoption sequence.

Remove the general migration/bootstrap/source-map/inventory/content-stage APIs with their consumers.
Do not implement the former conceptual `plan_migration`, `execute_migration`, `verify_migration` or
export pipeline just because an old function document listed them. New formats may open only new roots.

Remove the lifetime `OPERATIONS` archive and its generation-equals-history invariant. Where retries
are real, bound the retry window and protect only unresolved operations from eviction. Do not create
a general idempotency-maintenance subsystem to keep every historical command replayable.

The shared `search-publication` coordinator owns transitions; this package owns their persistence.
Remove the parallel daemon publisher/file journal. Optional secret/DEK operations do not belong to
the ordinary local-source transaction path.

## Verification

Check meaningful atomicity, conflict handling, known abort, unresolved commit, restart and
committed-snapshot recovery. Exercise pending Qdrant publication with the real adapter when claiming
that integration works. Verify that completed operation history cannot impose a lifetime update limit.
Keep source data intact when rejecting an old format. Old schema goldens, migration fixtures and
formal qualification records are not acceptance criteria for their replacement.
