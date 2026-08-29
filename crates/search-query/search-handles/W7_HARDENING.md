# W7 hardening — `search-handles`

This packet refines durable-handle authorization, lifecycle invalidation, purge and restore behavior.
The existing W4 `FUNCTIONS.md` remains authoritative for mint/resolve/expand basics.

## Durable eligibility

A durable handle is eligible only when all are exact and current:

- immutable admitted `SourceRevision` occurrence and residency closure;
- retained revision lease whose purpose includes durable handle;
- exact native anchor/excerpt digest and assurance ceiling;
- source namespace/owner generation and source/workspace view;
- binding/principal/grant plus access/security/purge generations;
- disclosure/range ceiling and finite TTL/quota;
- no unsaved/current-path target and no purge tombstone.

Eligibility receipt is consumed atomically with record creation. Losing the returned random token does
not permit blind mint retry; callers must treat mint as non-idempotent unless an explicit escrowed
operation profile is later admitted.

## `invalidate_lifecycle_scope`

```text
invalidate_lifecycle_scope(set, live_fence, store, operation)
    -> Result<HandleInvalidationReceipt, HandleError>
```

Invalidates exact records by owner/view/access/purge/residency/retention/buffer/profile scope under a
monotonic generation. Same operation/equal set is idempotent; a different set under the same identity is
rejected.

The receipt lists bounded affected/absent/already-invalid counts and digest, not plaintext tokens or
source metadata. Invalidation is published before acknowledgement.

## Purge

Purge invalidation permanently associates affected durable records/token digests with the tombstone
generation or deletes records according to policy while preserving non-content audit proof. Token replay,
backup restore, record import or remint cannot resurrect a target whose source/owner generation is
purged.

A handle invalidation receipt proves handle non-usability only. It does not prove Qdrant/CAS/backup
object deletion or physical secure erase.

## Expansion race hardening

Expansion must recheck live security:

```text
before record lookup response shaping
before revision readback
immediately after readback
before response emission
```

If the fence changes restrictively, bytes/readback are discarded and a binding-safe denial is returned.
A foreign/invalid/purged token must not disclose whether the record ever existed.

## Retention lease drift

Expired/released/mismatched retention lease, residency policy change or unavailable exact revision makes
the handle invalid. The package requests record invalidation and returns a typed gap; it never reads the
current path, extends retention silently or downgrades a durable handle to ephemeral.

## Restore/import

Server-owned handle records are not restored directly into serving state. Restore imports them only into
quarantine, then revalidates installation incarnation, token-digest schema, source/owner/revision,
residency, retention, binding/grant policy and every purge tombstone. Baseline policy should invalidate
old bearer tokens and mint new tokens after admission rather than reusing restored plaintext/opaque
handles.

Unsaved/ephemeral handles never survive restart or restore.

## Required tests

- durable eligibility fails each missing/mismatched prerequisite independently;
- random token/record redaction and non-idempotent lost-token behavior;
- purge/revocation/owner/residency/lease changes at every expansion checkpoint;
- readback completed then purge before emission returns no bytes;
- foreign/unknown/purged token has binding-safe indistinguishable failure shape;
- invalidation operation idempotency/conflict;
- tombstone blocks record restore/import/remint resurrection;
- restored bearer token is not automatically serving-valid;
- durable record cannot target unsaved bytes;
- handle receipt cannot satisfy projection/CAS/backup/secure-erase purge layers.
