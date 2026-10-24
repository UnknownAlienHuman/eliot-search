# search-control-redb

Typed current-state persistence for the local Eliot Search application.

Read the [architecture](../../docs/architecture/README.md),
[implementation guide](../../docs/implementation/README.md) and [source map](FUNCTIONS.md).

## Owns

Short redb transactions for small current records:

```text
AppliedSettings
RootRecord / SourceRecord / current RevisionRef
current PreparedArtifactRef
CurrentPublishedView
one PendingPublication
small schema / installation identity
```

The package may expose exact point reads and bounded current-table iteration for inventory/publication.
A narrow process-local pointer may mirror the committed current view or settings, but redb remains the
durable owner and restart reconstructs that pointer from typed rows.

## Does not own

- source bodies, prepared artifacts, manifest pages, vectors or Qdrant batches;
- completed-operation receipts, mutation history or a reconstructed in-memory control database;
- query windows, query history, grants, capability proofs or per-request checkpoints;
- old-format migration, cutover markers, generic quarantine, DEK/provider/security lifecycles;
- a second publication coordinator or generic snapshot publisher.

Immutable payloads live in the shared object store. Qdrant effects are executed and read back by the
publication owner outside a redb writer transaction. Only an external effect that can outlive the process
has durable pending state.

## Read path

1. Open one redb read transaction.
2. Verify the current schema/identity needed by the operation.
3. Point-read the requested typed keys, or iterate one bounded current table when the caller genuinely
   needs an inventory/publication snapshot.
4. Return values from one coherent MVCC view.

Do not scan historical operations or copy every control record for an ordinary read. The existing
`read_current_records_checked` implementation is a useful behavioral donor for finite point reads; the
replacement may use direct typed tables rather than its generic record encoding.

## Write path

1. Validate bounded input and expected-current values.
2. Perform source-object or Qdrant I/O outside the redb writer.
3. Open one short writer transaction over the affected typed rows.
4. Compare expected current state, write the replacement and commit.
5. Update a narrow in-memory pointer/cache only after commit.

A known abort is a known failure. If commit acknowledgement is genuinely uncertain, reopen and inspect the
affected typed row. Do not resolve uncertainty by scanning a permanent operation archive.

Publication is the one exception with durable external recovery:

```text
immutable bounded operation artifact
→ PendingPublication
→ exact Qdrant effects/readback
→ CurrentPublishedView compare/set
→ pending-state removal / cleanup cursor completion
```

## Remove after caller cutover

The current package still contains a generic `PersistentControlJournal`, `OPERATIONS`, full
`ControlSnapshot` reconstruction, publication-intent/visible-epoch compatibility schemas, access-policy,
quarantine, DEK/provider/security and migration modules. These are historical implementation sources, not
product requirements. Move useful codecs/primitives to their direct owner, migrate actual callers and
delete the generic platform and exclusive tests.

Unsupported pre-release roots fail before writes and remain untouched. No universal migration or backwards
compatibility programme replaces the deleted code.
