# Read-only journal health and counters

`write_counters_with_context` and `journal_health_with_context` use one coherent
redb read transaction per call. They read the existing header and table metadata,
not record bodies or the receipt inventory. No record, receipt, generation, owner,
quarantine marker, snapshot pointer or query-history entry is changed. Both calls
use one cooperative cancellation/deadline budget, including a final check before
returning an observation. A blocking native call cannot be preempted.

## Counters and their limits

`JournalWriteCounters` separates committed data generation and actual table
cardinalities from `acknowledged_mutating_calls`, the existing saturating per-handle
success counter. The latter includes initialization, owner handoff and hold writes,
resets on reopen, and does not count a commit whose acknowledgement was lost.
Neither value measures filesystem writes, flushes or OS I/O. Recovery is not
counted as another data mutation.

Header identity/schema/limits and actual record/receipt cardinalities are checked
before counters are returned. The byte totals are header declarations; exact
value/receipt-byte accounting remains the full verifier's responsibility. A
metadata-only check cannot detect arbitrary same-cardinality content tampering.
Normal counter reads refuse pending or quarantined journals. Exhausting ordinary
receipt capacity does not prevent a valid read-only diagnostic operation.

## Health is not readiness or recovery

Health can inspect the already-open journal while normal operations are blocked.
It reports pending mutation and local quarantine separately, with counters only
when the header and table metadata could be validated. Durable marker presence,
even if its bytes are malformed, is `DurablyQuarantined`, not healthy metadata.
A contradictory schema/header/table count is `QuarantineRequired`; the diagnostic
does not invent a durable hold receipt or silently repair the fault. Cancellation,
timeout and unavailable storage return errors rather than fabricated corruption.

`MetadataReadable` means exactly that. It is not a complete integrity check,
qualified H5 schema, accepted migration, native-root proof or permission to serve.
An unopenable database must still be inspected/excluded by its external owner;
this API does not reopen it or replace the outstanding side-effect-free
`inspect_journal` operation.

The optional publisher is observed, never updated. Health distinguishes an
unbound/model publisher, another root/owner binding, suspended publication, no
snapshot, and generations behind/aligned/ahead of the journal. Alignment does
not compare all snapshot content or grant admission. A previously cloned
publisher/Arc remains historical. A healthy-looking header never clears pending
mutation recovery; a usable old snapshot never overrides quarantine.

## Tests and integration boundary

Twenty regression tests cover initialization, replacements/deletes, replay,
reopen/owner handoff, lost acknowledgement, all publisher relationships, local
and malformed durable holds, metadata contradictions, the deliberate metadata-only
integrity ceiling, cancellation/deadline, unavailable inspection, full ledger,
redacted output and 10,000 repeated public health/counter calls without application
writes, value scans or pointer changes. Storage scenarios use real redb files;
checkpoint failures are explicitly test-only injection, not native fault proof.

```sh
cargo +1.98.0 test -p search-control-redb --lib --locked
cargo +1.98.0 check --workspace --all-targets --all-features --locked
```

Rust compilation and tests are NOT_RUN in this authoring environment: Cargo is
absent. Source/diff checks do not constitute execution or independent acceptance.
No shared port, schema, dependency, lockfile, workflow or daemon code changes in
this increment. T09 remains in progress; primary migration remains T10/T11.

The full publication-port integration has a separate load-bearing contract gap:
#141 records that `PublicationIntent.owner_source_membership_access_guards` is a
list of profile/auxiliary `StateDependency` values which cannot express the
required owner/source/membership/access generations. The existing coordinator
has a concrete `PublicationGuards` type; promoting it to the shared contract is
a breaking field-shape correction requiring explicit review/version treatment.
This increment does not silently make that change or replace the missing guards
with unrelated digests. H5 codecs, guarded VisibleEpoch operations, safe receipt
maintenance, native exclusion/inspection and executed qualification remain open.
