# Atomic durable VisibleEpoch commit

This increment implements the **durable control transaction** in Architecture 8.4
S13.4(8), not the Qdrant coordinator, installed product or complete ControlJournalPort.
It reuses the existing redb conditional transaction, replay and recovery engine.

## Exact atomic change

`VisibleEpochCommit` requires a READBACK_VERIFIED shared intent, its actual preceding
control commit, the exact expected route/visibility/guard record, producer-verified
new/retired point-manifest references and readback digest, and a bounded distinct
membership delta. A membership delta names its exact old/new manifest references
and either its exact source-revision/fence-revision shadow or required shadow absence.
There is no broad source/collection deletion filter.

The transaction compares the full intent, current visibility state (including all
seven PublicationGuards), old membership references and exact shadows. The current
header owner and control generation are independently checked. Only then does the
same transaction advance VisibleEpoch, replace/remove membership manifest refs,
retain retired refs under the publication receipt, remove only the matching shadows,
advance the shadow generation when removals occurred, change intent to CONTROL_COMMITTED,
and record the typed publication receipt plus ordinary idempotency receipt.

The publication receipt retains the original intent/manifest binding, collection and
schema identity, control operation identity, exact external readback references/digest
and actual intended next control generation. Its operation must exist in the same
coherent disk view. A caller-supplied prior receipt is compared with the actual ledger,
not accepted on its claimed revision or Boolean flags.

New, changed and removed manifest references remain technical CAS references. Their
contents, point IDs, source bodies and vectors are not inserted into control state.
The coordinator must verify that the delta is complete for the prepared CAS manifest
and that external stage/closure/readback evidence is real. This adapter neither reads
Qdrant nor manufactures that evidence. Source/access owners must update the stored
guard generations with their protected changes; producer composition remains required.

## Recovery and admission

Exact replay/recovery binds the whole generated command through the existing canonical
request fingerprint. Changed refs, shadows, guards, receipt IDs or readback digest cannot
replay the original request. Possible-write interruption retains the existing pending
fence. Recovery verifies committed post-state or absence; it never reconstructs a new
mutation from the current values. Historical replay after an admitted owner handoff does
not reapply old manifests/shadows; stale owners cannot make new forward progress.

Control commit does **not** acknowledge usable results or reopen a snapshot publisher.
The caller must serialize commit/publication with admission and use the existing exact
disk snapshot publication before acknowledgement. Failed or cancelled publication leaves
that publisher closed until verified snapshot recovery, without a second durable commit.

Schema-3 snapshot reads, verification and reopen validate visibility/intent/receipt
relationships. Missing state after initialization, a missing referenced receipt, a fake
CONTROL_COMMITTED intent, an unbacked receipt, wrong route/schema, malformed refs and
future publication receipts are rejected. Typed intent reads use that same validation.
Typed intent updates cannot erase an unresolved intent or bypass finalization; the raw
compatibility API is not a product publication path.

## Compatibility and deliberately unavailable paths

`PUBLICATION_VISIBILITY_SCHEMA_VERSION = 3` is explicit. Schemas 1 and 2 remain readable
under their previous rules and cannot call the visibility API. Existing files are not
upgraded. Prior binaries reject schema 3 rather than silently ignoring the new relationships.
The outer physical table/header/operation encoding is unchanged; new records use closed,
versioned private encodings. No dependency, lockfile, shared contract or workflow changes.

This remains the staged adapter layout, **not accepted H5 physical-table qualification**.
The H5 table migration and full closed port binding still require integration work.
Initialization is only for a genuinely new journal at epoch zero, with supplied identities
and complete guards; it is not a restore or namespace transfer. This implementation supports
the normal next-epoch path only. It deliberately does not implement skipped-epoch reuse,
abandon/invalidation-only finalization, route migration, successor reservation or reclaim.
The retained intent slot still cannot be overwritten by a second begin. These operations
must not be emulated by low-level raw writes or by changing an existing schema header.
The primary daemon has not been switched to this adapter.

## Verification boundary

Eighteen regression tests and one subprocess helper cover all guard axes, atomic manifest
and shadow changes, stale/forged receipts, skipped/incomplete commands, interruption,
reopen, lost acknowledgement, owner handoff, snapshot admission, missing/corrupt records,
strict codecs, bounds, an independently constructed byte fixture and 10,000 read-only calls.
The process fixture exits before/after the actual commit boundary with a finite parent
deadline; it is not a machine power-loss qualification. Fixed identities and references
are explicitly synthetic fixtures, not production evidence.

```sh
cargo +1.98.0 test --locked -p search-control-redb --lib
cargo +1.98.0 check --workspace --all-targets --all-features --locked
```

Rust compilation, tests, fmt and Clippy are NOT_RUN in the authoring environment.
Cargo was absent (actual check attempt exit 127); no native/backend acceptance is claimed.
Authoring checks covered exact base Git blobs, scoped edits, finite format arithmetic and
source structure, not executed Rust behavior.

The baseline package contains 8,163 raw Rust lines including comments/blanks/local tests.
This increment reaches 9,212, below the 10,000 hard ceiling but beyond the
8,500 split-review trigger. The integration split assessment keeps the new visibility
code/codec/tests together as a private transaction concern, with no new crate, mutable owner
or forwarding layer. Further H5/port expansion requires a reviewed size reduction or real
boundary split before crossing the hard ceiling. This assessment is not independent review
or an accepted package handoff; T09 and its build/qualification gates remain open.
