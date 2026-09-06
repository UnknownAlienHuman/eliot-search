# Durable administrative quarantine

`PersistentControlJournal::quarantine_with_context` records an explicit owner-requested hold.
The caller retains verified root exclusion and supplies the snapshot publisher used for admission.
A matching call immediately blocks both the journal and publisher, including on pre-cancellation.
A foreign publisher is rejected without changing either. Previously returned snapshot Arcs are not
revoked by Rust pointer mutation; composition must serialize admission and enforce live fences.

## Storage and compatibility

One fixed 145-byte `quarantine` record is added to the existing `eliot.control.meta.v1` table.
It contains `ELCTQ001`, a 32-byte operation ID, expected data generation (u64 BE), a closed reason
byte, the supplied diagnostic observation SHA-256, the exact header SHA-256 and a domain-separated
SHA-256 checksum. The supplied observation digest records the owner's diagnosis; it is not proof
that this module independently inspected the diagnosed external state. No paths, source/query text,
secrets, arbitrary reason strings or BLAKE3 relabelling enter the record.

This is an **optional administrative metadata-schema extension**, not a new H5 table inventory.
Healthy v1 journals remain unchanged. Data/header/operation codecs and their counters are unchanged.
A hold occupies its own fixed bounded slot, so a full ordinary receipt ledger cannot prevent it.
Its operation ID must not reuse a committed or pending data-mutation ID. Both the exact header and
operation-ID absence are rechecked inside the write transaction before recording the hold.

Old readers require exactly one META entry and reject the additional marker instead of ignoring it.
They cannot serve a held journal. New normal readers reject the mere presence of the marker, including
malformed or empty markers, as `CONTROL_STORE_QUARANTINED`. Exact diagnostic recovery performs strict
length, magic, reason, checksum, request, generation and identity/header-binding checks.
This is a corruption-detection checksum, not authenticated storage or protection against an attacker
with unrestricted file write access. Native ownership and file admission remain external obligations.

## Mutation and recovery

The marker is committed with immediate durability and exact readback. Ordinary records, data generation,
operation receipts, and any pending data-mutation identity are untouched. Same request replays without
another write; changed operation/reason/generation/observation conflicts. No API clears, overwrites or
prunes a hold. Owner handoff and snapshot recovery do not remove it.

Interruption before write dispatch creates no durable marker, but keeps process-local admission blocked.
Any failure after write dispatch requires exact recovery, even after a successful staged abort.
No returned durable receipt exists until readback completes. Process death after commit leaves the hold
visible to a newly opened process. An explicit hold is administrative state, not a fabricated ordinary
mutation receipt, new source generation or resolution of an earlier uncertain command.

`recover_quarantine_with_context` consumes an existing admitted file, reuses native open preflight,
and returns only the requested marker receipt or absence. It returns **no usable journal handle**.
Absence means only no marker was observed: it does not prove records healthy, permit normal serving,
resolve a data mutation, or establish owner authority. A mismatch fails closed.

Opening redb can recover native transaction metadata. Therefore this recovery is not side-effect-free
`inspect_journal`, and post-open cancellation/transient failure remains outcome-unknown. It performs no
application repair or marker/data writes. All checkpoints share one cooperative deadline; OS calls
cannot be forcibly interrupted.

The current adapter must be able to read the identity header, metadata and exact operation-ID slot to
record a hold. If those structures or the physical database are unreadable, it cannot manufacture a
durable marker receipt: it returns failure and leaves local admission blocked. Durable exclusion of an
unopenable database by the external root owner still needs composition. Existing automatically detected
corruption is not silently upgraded into a durable hold without an explicit owning call.

## Verification and integration boundary

Nineteen ordinary regression tests plus one ignored subprocess helper cover unchanged application bytes,
blocked snapshots/reads/mutations/open/handoff, exact replay and conflicts, full ledger capacity,
pre/post-dispatch cancellation, pending-operation preservation and ID collisions, malformed metadata,
foreign publisher/root, missing-table diagnosis, strict codec and process exit before/after commit.
The helper is invoked by its parent test; it is not a production fault switch. Process exit tests do
not constitute machine power-loss qualification. An independent SHA-256 known-answer fixture checks
the fixed marker layout.

```sh
cargo +1.98.0 test -p search-control-redb --lib --locked
cargo +1.98.0 check --workspace --all-targets --all-features --locked
```

Compilation and Rust/native execution are **NOT_RUN** in the authoring environment; Cargo was absent.
Static diff/format-structure/hash checks are not successful Rust tests. No T09 or product acceptance
is issued. Remaining T09 work includes full `ControlJournalPort`, semantic H5/VisibleEpoch codecs and
guards, side-effect-free inspection, safe idempotency maintenance, external quarantine composition and
executed qualification. Primary daemon migration remains T10/T11; no daemon code changes here.
