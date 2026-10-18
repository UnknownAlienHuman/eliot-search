# Local control storage

Use the [implementation guide](../../docs/implementation/README.md#3-keep-control-state-small-and-publication-real).
This package is the concrete redb owner for current source/settings/publication metadata.

## Write the useful state

Use typed methods and short redb transactions. Read conditions inside the write transaction and commit
the changed tables. Keep read snapshots coherent. Do not hold a write transaction over Qdrant RPCs.

Remove the generic per-update plan/fingerprint/receipt/readback platform and permanent `OPERATIONS`
archive. Its 65,536-operation baseline cap must not become a lifetime limit on a working root.
Only real pending external work and a bounded retry window where needed belong in recovery state.

A successful abort before commit means the local writes were rolled back. A failed commit or possible
external Qdrant effect is a different case. Do not demand recovery receipts for every cancelled read.

The shared publication owner uses actual persistent pending/visible transactions here.
`publication_codec::cas` is a pure floor check, not durable commit; retire its duplicate FileJournal
and daemon publication path after transferring useful behavior.

## Storage boundary

Source bodies, prepared text/maps, vectors, postings and query/ranking history do not belong in control
tables. Immutable objects live with the revision/preparation owner; Qdrant owns indexed retrieval.

The new application may reject an unsupported old root before writing and leave it intact. General
migration staging/cutover/rollback, old-format codecs and a new export framework are not prerequisites.
Historical README/FUNCTIONS schemas explain existing code; they do not reinstate retired requirements.

Keep actual format/transaction/source-reference/publication consistency and actionable scoped errors.
Test committed-state reopen, abort, genuine unknown outcomes and bounded retry-state retirement.
Delete old ledger/migration fixtures with their obsolete mechanism; no byte-stable receipt output or
prescribed static gate is required.
