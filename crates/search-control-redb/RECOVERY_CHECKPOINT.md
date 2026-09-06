# Coherent publication recovery checkpoint

`read_publication_checkpoint(context)` reads the schema-3 route/visibility record,
last retained intent and bounded operation ledger through one redb read transaction.
It returns a package-owned immutable `PublicationRecoveryCheckpoint`, not a restored
coordinator, a new owner lease, a published snapshot or permission to use the index.

The checkpoint exposes the exact journal identity, control generation, visibility
state, last intent and consumed reservation floor. The floor comes from the retained
intent even when VisibleEpoch is lower. For example, a durable aborted intent at
epoch 1 with visible epoch 0 still reports a consumed floor of 1 after reopening.
The empty case is accepted only when the existing loss-detection checks establish
that the intent slot was never written. Missing route, missing-after-write intent,
contradictory receipt or damaged ledger is an error, not an epoch reset.

Only an explicitly initialized empty schema-3 journal returns `None`. A route
initialized at epoch zero returns a checkpoint with no intent and floor zero.
Schemas 1 and 2 are refused without migration or mutation. Existing schema-3 bytes,
keys, codecs and receipt identities are unchanged. No separate reservation ledger,
new database, serialization format, schema version or dependency is introduced.

This is a recovery-plane operation: it performs bounded full consistency/ledger
verification, then reuses the existing typed relationship validator within the same
read transaction. It is not a replacement for snapshot-based hot query admission.
All checks share one cooperative deadline. Cancellation, an uncertain mutation or
quarantine yields no partial checkpoint and does not clear any existing hold.
The returned value is historical once the journal changes. Its construction is
crate-restricted; Debug excludes manifest references and intent contents.

## A terminal label is not completed external recovery

Previously `load_unresolved_publication` omitted any ABORTED intent and disk control
snapshot reconstruction accepted that state. A plain intent update could therefore
remove this admission barrier without a bound durable compensation or exclusion
receipt. The old test explicitly expected that unsafe behavior.

For the current staged schemas, ABORTED now remains recovery-visible and blocks
control snapshot reconstruction/publication. Reading a checkpoint still exposes its
exact epoch and original guards for recovery. Old aborted records remain readable;
they are not erased, rewritten, declared corrupt or turned into new intent slots.
Ordinary `begin` and normal committed-successor reservation still cannot replace them.
This tightens the old admission behavior without changing the enum or disk codec.

A complete abnormal-finalization operation must later bind verified two-sided index
compensation or effective membership-wide pre-retrieval/pre-IDF exclusion to durable
control state before clearing this hold. No new automatic resume, invalidation-only
commit, skipped-epoch commit or Qdrant repair is authorized here. The full H5 schema,
ControlJournalPort and primary daemon migration remain separate unfinished work.

## Verification

Seven new real-redb regression fixtures cover empty/legacy schemas, coherent floor
readback and reopen, aborted-state blocking after reopen, missing route/intent,
unknown-write recovery and owner handoff, a single snapshot read per checkpoint,
10,000 calls without application writes, redacted output and shared cancellation/
deadline behavior. The existing schema-2 abort test now requires the recovery fence;
other existing tests and process helpers are retained. Fixture identities are
synthetic: they do not establish native ownership or external index correctness.

```sh
cargo +1.98.0 test --locked -p search-control-redb --lib
cargo +1.98.0 check --workspace --all-targets --all-features --locked
```

Rust compilation, tests, formatting and Clippy were NOT_RUN in the authoring
environment: Cargo is absent (preflight exit 127), and local download attempts
failed DNS resolution. Source/diff checks do not replace executed qualification.
No task acceptance, independent review or runtime readiness is claimed.

Normative basis: S13.1/S13.5/S13.6 and the package [function contract](FUNCTIONS.md).
Related boundaries: [intent storage](PUBLICATION_INTENTS.md),
[normal succession](SUCCESSOR_RESERVATION.md) and [visibility commit](VISIBLE_EPOCH.md).
