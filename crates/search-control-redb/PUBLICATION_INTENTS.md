# Typed durable publication intents

`PublicationIntentUpdate` records the shared `PublicationIntent` in the existing
`PersistentControlJournal`. It uses the same conditional transaction, receipt,
cooperative deadline and exact-request recovery as ordinary control mutations.
There is no second database or publication coordinator.

## Operations

- `begin` records the coordinator's PREPARED input as INTENT_DURABLE.
- `advance` uses `search_domain::transition_publication`, preserving identity,
  target epoch, manifest reference and all seven guards. The complete previous
  intent and expected journal generation are atomic preconditions.
- `read_publication_intent` returns the last exact value and its coherent control
  generation. `load_unresolved_publication` keeps a bare ABORTED value visible for recovery.
- `read_publication_checkpoint` returns coherent route/visibility, intent and consumed
  floor from one schema-3 read transaction; it never acknowledges external recovery.
- `persist_publication_intent` and `recover_publication_intent` bind the complete
  command to the existing idempotency ledger. Replay does not reapply pre-state.

A second begin cannot overwrite the retained slot. Terminal records are not deleted.
If the slot disappears after a receipt recorded its write, reads, verification,
reopen and further typed writes fail instead of returning a successful empty result.
The absence check scans the bounded receipt ledger only when the slot is absent;
normal initialized intent reads do not scan that ledger or unrelated values.
Safe receipt pruning must preserve this loss-detection evidence or explicitly migrate
it before removal; no pruning is introduced here.

New forward progress requires the prepared owner epoch to equal the journal owner.
An explicitly admitted successor may record compensation/block/abort while preserving
the old guards. Exact historical replay remains possible after owner handoff.
These checks do not replace the externally held root-owner guard.

## Visibility and recovery

The ordinary intent writer cannot set CONTROL_COMMITTED,
INVALIDATION_ONLY_COMMITTED or RECLAIMABLE. Those transitions require the complete
VisibleEpoch/finalization operation; raw lower-level compatibility APIs are not
an alternative product publication API.

Recovery-required records, including READBACK_VERIFIED, COMPENSATING,
PUBLICATION_BLOCKED and a bare ABORTED value, block disk control-snapshot
reconstruction/publication. ABORTED alone does not prove two-sided compensation
or complete effective exclusion. Existing aborted records remain readable and
retain their consumed epoch; they are not corruption or empty reusable slots.
See [recovery checkpoint](RECOVERY_CHECKPOINT.md).

CONTROL_COMMITTED resolves the durable mutation and allows snapshot reconstruction;
it does not itself reopen publisher admission. Publication acknowledgement still
follows verified snapshot publication. This avoids a circular requirement to publish
a snapshot before its already committed control record can be read.

The coordinator must supply actual external stage/closure/compensation evidence.
Persisting a state does not manufacture that evidence, verify Qdrant or grant access.
Interrupted writes retain the existing unknown-outcome fence until exact recovery.
One deadline covers validation, inspection and transaction work; native I/O remains
cooperatively, not forcibly, interruptible.

## Explicit schema boundary

Typed intent operations accept adapter schemas 2 and 3. Schema 1 remains supported
unchanged for existing low-level callers but cannot invoke typed-intent operations.
Visibility and coherent publication checkpoints require schema 3. No existing header
is upgraded or reinterpreted; creation at a supported schema is explicit and migration
remains a separate unimplemented operation. Supplying a different expected schema for
an existing file fails header validation. Implementations predating these schemas
reject them instead of silently ignoring unresolved state.

The outer table/header/receipt layouts are retained; the identity's schema version
changes. The private intent value has `ELIPUB01` magic, fixed-width big-endian fields,
closed state tags and one bounded length-prefixed `ReceiptRef`. Decode rejects missing
fields, unknown version/state, invalid epochs/owner, invalid UTF-8 and trailing bytes.
The only textual field is an opaque producer-validated reference, not a free-form
payload. Its semantic validity remains the reference owner's obligation. No point-list,
source-body, vector or query field is added. Debug omits reference contents.

This is the typed-intent part of T09, not an accepted H5 schema. Schema-3 normal
visibility commit and [committed succession](SUCCESSOR_RESERVATION.md) now have separate
typed operations. Full H5 tables, aborted/skipped-epoch finalization, migration and
primary daemon integration remain unfinished. Ordinary `begin` still cannot replace
the retained slot, including after abort. No gate or readiness flag is advanced and
no dependency or workflow is changed.

## Verification

Package tests cover exact field/state round trips, malformed encodings, atomic conflicts,
restart, bounded process-exit fixtures, lost acknowledgements, interruption, stale-owner compensation, lost/corrupt
records, snapshot blocking, schema mismatch and 10,000 nonmutating typed reads.

```sh
cargo +1.98.0 test --locked -p search-control-redb --lib
cargo +1.98.0 check --workspace --all-targets --all-features --locked
```

Rust compilation, tests, formatting and Clippy are NOT_RUN in the authoring environment:
Cargo/rustc are absent and download access failed. Local byte-layout and source-diff
checks are not executed Rust tests or independent acceptance.
