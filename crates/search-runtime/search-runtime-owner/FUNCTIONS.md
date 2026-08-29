# Function contract — `search-runtime-owner`

**Status:** W1/P01 logical contract; no owner implementation or Windows qualification evidence exists.

This package is the sole owner of data-root acquisition, owner-epoch fencing, abandoned-owner
classification and clean release. It owns no daemon composition, control tables, source state or Qdrant
process lifecycle.

## Global rules

- one canonical local data root has at most one live owner incarnation;
- path text, lock-file presence, PID or responding endpoint alone never proves ownership;
- every successful mutation binds installation incarnation, process creation identity, owner epoch,
  mode and canonical data-root identity;
- ambiguous state quarantines rather than attaches or deletes another process record;
- equal operation identity plus equal canonical request is idempotent; conflicting reuse is rejected;
- public diagnostics expose location class/digest and identity state, not unrestricted absolute paths.

## Configuration operations

### `section_descriptor() -> ConfigSectionDescriptor`

### `compiled_defaults() -> ConfigSectionInput`

### `validate_section(input, platform, accepted_capabilities) -> Result<ValidatedInstanceConfig, OwnerError>`

Implements `config/sections/instance.md`. Rejects remote/network roots, invalid mode, unsafe owner-recovery
policy and bounds outside the packet. Mode or data-root changes cannot apply live.

### `section_digest(validated) -> Blake3Digest32`

### `plan_section_change(old, new) -> Result<SectionReloadDecision, OwnerError>`

Preserves `DRAIN_AND_RESTART` for mode/data-root changes and every stricter obligation. This package does
not execute daemon restart or publish a global config snapshot.

## Identity and path preparation

### `resolve_data_root(request, platform) -> Result<ResolvedDataRoot, OwnerError>`

Canonicalizes the requested or OS-default local root, validates local-volume policy, obtains stable path/
volume/file identity where supported and produces a content-minimized identity digest.

It does not create ownership state. Reparse/device/network ambiguity fails closed.

### `build_owner_identity(process, executable, installation, mode) -> Result<OwnerIdentity, OwnerError>`

Requires process creation identity/handle semantics, executable file identity/digest, installation
incarnation and exact mode. PID reuse cannot satisfy equality.

## Owner acquisition

### `inspect_existing_owner(root, observer, deadline, cancel) -> Result<OwnerObservation, OwnerError>`

Reads the durable/OS ownership markers and observes the referenced process without mutating state.
Returns one closed classification:

```text
ABSENT
LIVE_MATCHING_OWNER
LIVE_CONFLICTING_OWNER
STALE_RECORD_IDENTITY_ABSENT
AMBIGUOUS
CORRUPT
```

Cancellation before a complete observation returns no reusable ownership claim.

### `acquire_data_root(request, owner_identity, operation, deadline, cancel) -> Result<OwnerGuard, OwnerError>`

**Preconditions**

- resolved root and owner identity are valid;
- no unresolved acquisition/release operation uses the root;
- mode is compatible with the requested composition profile;
- cancellation/deadline are explicit and finite.

**Postconditions**

- one OS ownership primitive and one durable owner record agree on root, incarnation, process and epoch;
- owner epoch is fresh and strictly greater than any safely observed abandoned generation;
- returned guard is non-serializable and is the only package authority for subsequent owner mutations;
- a second concurrent acquisition is denied.

Cancellation before mutation is clean. Timeout/cancellation after the OS/durable mutation boundary is
`OWNER_ACQUIRE_OUTCOME_UNKNOWN`; recovery uses exact ownership observation and the original operation
identity, never a second blind acquire.

### `verify_owner_guard(guard, current_process, current_root) -> Result<OwnerVerificationReceipt, OwnerError>`

Rechecks owner epoch, installation/process/executable/root identity and OS ownership primitive. Mismatch
invalidates the guard and requests quarantine.

## Recovery

### `classify_abandoned_owner(observation, policy) -> RecoveryDecision`

Purely maps a complete observation to:

```text
START_FRESH
DENY_LIVE_OWNER
CLEAN_STALE_RECORD_AND_RETRY
TERMINATE_VERIFIED_ORPHAN_THEN_RETRY
QUARANTINE
```

Baseline policy never blindly attaches. Termination is legal only for an exact verified owned orphan and
is performed by an injected platform/process port, not inferred from PID.

### `recover_acquisition(root, operation, expected_owner, observer) -> Result<OwnerGuard, OwnerError>`

Resolves unknown acquire outcome by exact OS/durable readback. It reconstructs the guard only when every
identity matches; conflicting/partial state quarantines.

### `recover_release(root, operation, expected_owner, observer) -> Result<OwnerReleaseRecovery, OwnerError>`

Distinguishes still-owned, cleanly released, partially released and ambiguous state. It never deletes a
record that may protect a live process.

## Drain and release

### `begin_drain(guard, reason, operation) -> Result<DrainToken, OwnerError>`

Atomically changes package owner lifecycle from `ACTIVE` to `DRAINING` for the exact guard. Idempotent for
the same operation/reason. It does not drain requests or dependencies; daemon orchestration owns that.

### `verify_release_preconditions(guard, drain_token, dependency_receipts) -> Result<ReleasePermit, OwnerError>`

Requires matching owner/drain identities and exact receipts proving endpoint/control/source/index/process
layers have been stopped according to the composition profile. Missing or mismatched receipts reject
release.

### `release_cleanly(guard, permit, operation, deadline) -> Result<OwnerShutdownReceipt, OwnerError>`

Removes/marks the durable owner record and releases the OS ownership primitive in a crash-recoverable
order. Success proves this owner no longer claims the root; it does not claim process-wide secure erase.

Same operation is idempotent. Timeout after either mutation is unknown until `recover_release`.

## Health and diagnostics

### `owner_health(guard, observer) -> OwnerHealth`

Returns lifecycle/identity consistency, owner epoch, location class/digest and bounded reason codes only.
It contains no secret, source content or unrestricted path.

## Cancellation, deadline and crash semantics

All platform observations are deadline/cancellation aware. No cancelled partial observation becomes an
owner proof. Every acquire/release mutation has a stable operation identity and exact readback recovery.
Process crash leaves OS/durable evidence for the next owner to classify; it never automatically makes a
root safe to reuse.

## Typed failures

- `DATA_ROOT_INVALID`
- `DATA_ROOT_REMOTE_DENIED`
- `DATA_ROOT_ALREADY_OWNED`
- `OWNER_MODE_CONFLICT`
- `OWNER_EPOCH_MISMATCH`
- `OWNER_PROCESS_IDENTITY_MISMATCH`
- `OWNER_EXECUTABLE_IDENTITY_MISMATCH`
- `OWNER_IDENTITY_AMBIGUOUS`
- `OWNER_ACQUIRE_OUTCOME_UNKNOWN`
- `OWNER_RELEASE_OUTCOME_UNKNOWN`
- `OWNER_RECOVERY_QUARANTINED`
- `OWNER_DRAIN_REQUIRED`
- `OWNER_RELEASE_PRECONDITION_MISSING`
- `OWNER_OPERATION_CONFLICT`
- `OWNER_CANCELLED_BEFORE_MUTATION`

## Required tests / qualification evidence

- OS-default/local-root canonicalization and remote/reparse/device denial;
- two-process concurrent acquisition: exactly one succeeds;
- PID reuse, executable replacement and installation-incarnation mismatch;
- stale record with proved absent identity cleans safely;
- ambiguous process/root identity quarantines;
- crash before/after each OS/durable acquire and release boundary;
- same-operation replay reconstructs guard/receipt; conflicting replay rejects;
- standalone/managed co-ownership and live mode transition rejected;
- owner epoch monotonic across crash/reopen and never reused;
- release requires drain plus dependency shutdown receipts;
- debug/serialization/path disclosure audit;
- `instance` configuration validation/change classification;
- fake platform/process/record ports prove portable decision logic.
