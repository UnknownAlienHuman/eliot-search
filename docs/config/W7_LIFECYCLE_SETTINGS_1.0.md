# W7 lifecycle settings 1.0

Machine schema: [`../../config/w7-lifecycle.toml`](../../config/w7-lifecycle.toml).

## Principle

Configuration tunes finite resource/scheduling policy. It cannot create a purge or restore command,
remove a tombstone, weaken current authorization, delete client-owned evidence, bypass mark/pin roots or
turn a quarantined restore into serving state.

Fields are classified as:

- `LOCKED` — architecture/security invariant; override is rejected;
- `TUNABLE` — bounded resource/scheduling value owned by one capability;
- `QUALIFIED_REF` — opaque accepted provider/artifact reference;
- `COMMAND_ONLY` — authenticated explicit lifecycle command, not a persistent config value.

## Security and invalidation

Restrictive state is monotonic. `ack_after_live_fence=true` and whole-leg contamination discard are
locked. Batch/time settings may change live inside bounds, but a smaller timeout cannot convert an
unknown committed restriction into rollback or reopen access.

Invalidation targets are exact owner/view/access/purge/route/profile identities. No config field enables a
broad vendor filter or skips an owner receipt.

## Handles and continuations

Durable handle eligibility, immutable retained revision, no-unsaved rule and restored-token non-reuse are
locked. Continuations cannot persist process-local pins, vendor cursors or unsaved bytes. Config may
adjust future invalidation batch sizes only.

Lower quotas/TTLs in existing owner sections trigger explicit invalidation/expiry work; records are not
silently reclassified or restored.

## Revision store

Residency separation, exact lifecycle authority, purge-tombstone enforcement and restore quarantine are
locked. Object inventory page size is tunable. Lower storage quotas do not directly delete objects; they
schedule a new retention evaluation.

## Retention

All architecture-required roots and fresh active-pin protection are locked. Reference count is never
deletion authority. Mark/sweep batch sizes, background budget and interval are finite tunables.

Changing retention policy/domain/legal hold is not a simple live scalar update; it requires an explicit
security-barrier/policy transaction and a new sweep generation. The old protection set remains effective
until the transition receipt is accepted.

## Purge

Purge is `COMMAND_ONLY`. A config file, environment variable or CLI settings flag cannot create a purge
operation. Locked rules require live fence/tombstone before acknowledgement/destructive work, prevent
later layer failure from reopening access, reject ordinary reclaim as purge and prohibit secure-erasure
overclaim.

Only bounded layer batch sizing is tunable. It does not alter scope, authority, receipts or tombstone.

## Backup and restore

A backup provider may be referenced only by accepted `QualifiedBackupProviderRef`; absence means backup
or provider-specific deletion status is `UNAVAILABLE`, not success.

Restore is `COMMAND_ONLY`. Paired manifest, quarantine, current source/access/residency/profile/purge
revalidation and new guarded publication are locked. Old backup visible epoch/tokens/handles never become
serving merely because bytes were restored.

## Composite reconfiguration

Possible obligations include:

```text
APPLY_LIVE
SECURITY_BARRIER
RESTART_DEPENDENCY
DRAIN_AND_RESTART
REBUILD_PROJECTION
NEW_COLLECTION_GENERATION
GATE_REQUIRED
REJECT
```

Multiple obligations may coexist. A candidate global config fingerprint becomes authoritative only after
all required owner receipts succeed. Failure preserves the previous effective snapshot and the lifecycle
security state remains fail-closed.

## Redaction

Diagnostics may expose section/field, owner, value class, provenance, bounds, action and reason code.
They exclude secret refs unless required, secret bytes, source/query content, handle/continuation tokens,
raw object paths, client evidence identities and backup credentials.

## Required tests

- every field has one owner/type/mode/default and bounded action;
- every locked field rejects file/environment/CLI override;
- `COMMAND_ONLY` purge/restore cannot be materialized from configuration;
- unqualified backup provider ref rejected;
- restrictive change retains all security/invalidation obligations;
- lowering batch/timeout never widens scope or converts unknown outcome to success;
- retention/root/pin/tombstone/quarantine floors cannot be disabled;
- ordinary reclaim/CAS sweep cannot satisfy purge;
- secure erase and client-evidence deletion claims stay false;
- failed composite apply preserves prior config and all monotonic fences;
- redacted view leaks no content, tokens, secrets or unrestricted paths.
