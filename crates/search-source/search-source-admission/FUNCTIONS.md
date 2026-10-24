# search-source-admission — source and deletion map

## Current connected call

`bins/eliot-searchd/src/owner_composition/kernel/native_source.rs::acquire_source_v2` currently performs:

```text
native root/file qualification
→ construct metadata input
→ reconstruct canonical AdmissionObservationV2
→ evaluate_pre_read_v2
→ stable safe-reader body acquisition
→ hash/classify body
→ reconstruct another canonical observation
→ OriginalAdmissionV2::retain
   → full evaluate_v2
   → receipt_v2 issue/verify
   → retained original policy/observation/decision/receipt
→ derive another native observation digest and source identity
→ return AcquiredNativeSource without the body
```

`AcquiredNativeSource` retains native handles, identity/binding observations and `OriginalAdmissionV2`; it
cannot pass the exact bytes to the object writer. Registry/source-record v4 codecs then persist/reconstruct
the original-input archive.

## Useful current functions

| Source/API | Disposition |
|---|---|
| `policy_v2::compile_policy_v2` and reviewed `globset` matching | Keep/simplify as the one current policy compiler. |
| `evaluation_v2::evaluate_pre_read_v2` | Keep the behavior, with a smaller current observation type. |
| `evaluation_v2::evaluate_v2` | Reduce to a post-read finalizer over actual acquired-byte facts. |
| `observation_v2` | Replace with small before/after structs; no canonical fingerprint authority. |
| `batch_v2::evaluate_batch_v2` | Delete; inventory calls the pure evaluator per candidate or in an internal iterator. |
| v1/bootstrap/original envelope/input models | Delete after legacy readers are removed. |
| receipt/provenance packages | Delete from current ingestion; they add no read/storage/currentness guarantee. |

## Target data

```text
CompiledAdmissionPolicy {
  revision
  finite allow/deny/review rules
  maximum admitted bytes/classes/locations
}

CandidateBeforeRead {
  root_relative_locator
  source/file class
  location class
  observed metadata required by policy
}

CandidateAfterRead {
  exact byte length
  binary/text/encoding/content class
}

AdmissionDecision {
  outcome
  stable reason code
  final classification when allowed
  policy revision
}
```

Do not copy source bytes or native handles into policy values. Do not include caller-produced digests,
`accepted` Booleans, access grants, source identity or currentness.

## Direct ingestion sequence

```text
compile policy once for inventory/config revision
→ native owner builds CandidateBeforeRead
→ pre_read
→ if allowed: one stable final-handle read
→ native/source owner computes actual body facts
→ finalize
→ if allowed: pass same owned body to #330 object writer
→ compare/set typed source head under #270
→ enqueue #331 preparation
```

A post-read decision may narrow a pre-read allow. It cannot widen a deny/review/unsupported/incomplete result.
Cancellation/deadline before object publication retains nothing. Once an immutable object is published,
failure to commit the source head leaves a safe unreferenced object for bounded GC.

## Persistence

Store only policy revision, outcome/reason and selected classification when useful for current diagnostics.
No complete original policy/observation array, receipt fingerprint, matched-rule archive or completed operation
is required in redb.

An old root which depends on v1/v2 archive bytes is unsupported by the new application. Refuse it before
writes and leave it intact. Do not port archive fragments or source-record v4 batches into the current schema.

## Delete with callers

```text
search-source-admission:
  bootstrap_v1
  batch_v2
  original_envelope
  original_inputs_v2
  v1 families and receipt-oriented portions of lib.rs

search-source-admission-receipt package
search-source-provenance package
source-records provenance/v4 archive codecs
registry_control original_inputs_v4 / v4_restore / batch restore paths
old receipt/archive/canonical/batch fixtures and docs
```

Move only a genuinely useful bounded policy type/reason code to the source owner. No forwarding modules or
`admission-v3` compatibility family.

## Verification

- deny/review/unsupported/incomplete performs no body read;
- known private-key/credential/system/own-state exclusions remain fail-closed in the correct native/policy
  owner;
- allowed body is read exactly once and retained unchanged;
- post-read binary/encoding/size classification narrows correctly;
- policy update is reflected on the next inventory/reconcile without operation-history replay;
- restart point-reads current source/head and never restores original admission inputs;
- ordinary errors reveal no locator/rule/source content.
