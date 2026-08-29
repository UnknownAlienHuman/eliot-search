# W7 security lifecycle, retention, purge and restore qualification contract

**Status:** `NOT_EXECUTED`  
**Architecture:** ELIOT Search 8.4, S13–S14, S23, S26–S28, H4, H6, H11–H16, P13  
**Scope:** restrictive mutation linearization, active-request checkpoints, durable handles and
continuations, source-backed emission, CAS mark/sweep, purge, paired recovery and restore quarantine.

Passing unit tests, deleting a Qdrant point, expiring a cache entry or restoring one database is not
qualification. Every mandatory probe in [`probes.toml`](probes.toml) must execute against exact accepted
package/API/configuration/fixture/provider identities and receive independent review.

## Owners

| Evidence | Owner |
|---|---|
| restrictive mutation, live security fence and contamination decisions | `search-access` |
| durable handle eligibility, expansion reauthorization and invalidation | `search-handles` |
| continuation invalidation/pin/dependency hardening | `search-continuation` |
| source-backed candidate lifecycle checkpoints | `search-candidate-validator` |
| immutable objects/revisions/leases/exact object lifecycle operations | `search-revision-store` |
| retention roots, mark/sweep policy, purge, tombstones and restore quarantine | `search-retention` |
| invalidation-only publication and purge/restore publication interaction | `search-publication` |
| ordinary exact retired-point reclaim boundary | `search-index-reclaimer` |
| active pin snapshots/watermarks | `search-epoch-pins` |
| concrete control/object/index/secret/process adapters | `eliot-searchd` composition only |
| evidence aggregation and leakage audit | `search-eval` |
| final acceptance | integration owner + independent reviewer |

A producer cannot accept its own evidence. Missing owner receipt keeps the affected domain fail-closed.

## Frozen inputs

Before execution publish immutable identities for:

- repository commit, Rust toolchain and `Cargo.lock`;
- accepted W0–W6 contract/domain/port/source/query/publication/pin/handle/API digests;
- effective configuration and lifecycle settings fingerprints;
- control schema/migration and data-root owner epoch;
- object/revision/manifest/residency schemas and fixture digests;
- Qdrant server/client/collection/profile and publication receipts;
- active pin, access/security/purge and source/workspace view fixture identities;
- backup/recovery provider or explicit `UNAVAILABLE` status;
- fault driver, filesystem/platform/resource envelope and every probe byte digest.

Mutable branches, unspecified backup semantics, broad deletion scripts and locally edited fixtures are
invalid inputs.

## Execution order

1. Execute restrictive mutation control-commit, immutable snapshot publication and invalidation receipt
   recovery across every crash boundary.
2. Revoke/purge at each request, retrieval/IDF, readback, validation, handle, continuation, exact and
   emission checkpoint.
3. Execute durable handle/continuation eligibility, token privacy, lease drift and restart/restore cases.
4. Exercise revision-store immutable write/readback/lease/root/inventory/tombstone/quarantine ports.
5. Freeze one complete durable root set plus fresh active-pin protection and execute multi-slice mark.
6. Exercise mark checkpoint/crash/resume, root/hold/pin drift and exact bounded sweep.
7. Install a live purge fence before acknowledgement and run every purge layer with crash/timeout/
   partial/unavailable outcomes.
8. Verify logical denial remains complete while physical/cache/backup layers may remain partial and
   secure erase is not overclaimed.
9. Attempt reindex, handle remint and restore resurrection against tombstones.
10. Build a paired redb/Qdrant/object recovery manifest, restore into quarantine and exercise all
    pairing/source/access/residency/profile/purge mismatches.
11. Revalidate/rebuild and commit one new guarded route; prove old backup visibility never serves.
12. Audit the ordinary-reclaim versus purge receipt/type boundary.
13. Publish immutable raw outputs and independent review. Only then may P13 evidence be accepted.

## Mandatory properties

### Restrictive security and active requests

- restrictive control state is monotonic and durable;
- acknowledgement waits for live immutable security-fence publication;
- crash/timeout after possible control commit is recovered by operation/generation readback;
- live checks occur at admission, before/after every scoring/IDF leg, before/after source readback,
  before emission, before/after handle expansion and every continuation/exact/restore admission step;
- a restrictive population change discards the whole contaminated leg/result unit, not one candidate;
- missing invalidation owner receipt keeps the scope fail-closed;
- permissive policy never removes a purge tombstone or resurrects content.

### Handles, continuations and candidate emission

- durable handle requires immutable admitted retained revision plus current owner/view/access/residency/
  purge/disclosure/TTL/quota eligibility;
- possession never grants access and foreign/unknown/purged token failure does not disclose existence;
- expansion rechecks before lookup shaping, before and after readback and before emission;
- purge/revocation after readback but before emission returns no bytes;
- durable continuations own no process-local pin, vendor cursor, source body or unsaved snapshot;
- ephemeral handles/continuations and unsaved targets never survive restart/restore;
- restored durable records remain quarantined and old bearer tokens are not automatically serving-valid;
- candidate validator rejects physically present/restored purged or old-epoch material.

### Retention and mark/sweep

- protection roots include every architecture-required durable root and fresh active-pin protection;
- sweep uses one frozen root/control/policy/pin generation and exact object inventory;
- missing/corrupt root/manifest/pin state blocks deletion;
- reachability is manifest-graph based; reference count alone is never deletion authority;
- shared objects remain while any membership/publication/handle/job/import/hold root reaches them;
- partial/cancelled mark cannot authorize sweep;
- new root/hold/pin/purge generation invalidates or narrows in-progress sweep;
- delete batches contain exact object IDs and use unknown-outcome readback;
- final receipt accounts every planned/deferred/protected object and claims ordinary CAS/cache sweep only.

### Purge

- live deny fence and tombstone are committed/published before purge acknowledgement or destructive work;
- failure of projection/cache/CAS/backup deletion never reopens logical access;
- handle/continuation/candidate/cache/client invalidations require owner receipts;
- security-purge index path/receipt is distinct from ordinary reclaim;
- a shared physical object may remain while the purged scope is logically inaccessible;
- purge layer statuses remain independently visible;
- Search reports physical secure-erasure limitations rather than claiming guaranteed erase;
- client revocation event does not claim authority over client-owned canonical evidence;
- tombstone blocks reindex, remint, import and restore resurrection;
- repeated equal operation is idempotent; conflicting operation reuse is rejected.

### Backup and restore

- backup/recovery uses one paired manifest binding redb checkpoint, Qdrant snapshot, object roots,
  schema/profile, committed epoch/publication and purge-tombstone generation;
- redb-only or Qdrant-only restore is never serving-capable;
- restore target begins quarantined/non-serving and stays so across restart/failure;
- every external source identity/owner/membership/access/residency/revision/profile and purge tombstone is
  revalidated;
- purged material is excluded before any reindex/readback/publication;
- missing/drifted material becomes explicit rebuild/drop gaps;
- old backup visible epoch/route is never directly restored as current;
- serving admission requires exact rebuild/readback and one new guarded publication/route commit;
- restore cancellation/crash remains quarantined.

### Boundary and receipt truthfulness

- ordinary point reclaim, object sweep, security purge, backup deletion and secure erase have different
  operation/receipt classes;
- no receipt may satisfy another layer merely because the bytes are absent;
- vendor/database types and broad correctness deletion predicates never cross public lifecycle ports;
- diagnostics/evidence are content-minimized and exclude source/query bytes, secrets and unrestricted
  paths.

## Stop conditions

Any of the following keeps P13 unavailable:

- restrictive command acknowledged before live fence publication;
- candidate/score/count/trace emitted from a contaminated leg;
- revocation/purge race passes any mandatory checkpoint;
- handle/continuation token existence leaks across bindings;
- durable record targets unsaved or unretained content;
- missing root/pin/manifest state permits sweep;
- reference count alone authorizes deletion;
- partial mark or stale root/pin generation authorizes sweep;
- ordinary reclaim receipt accepted as purge;
- logical access reopens because a later purge layer failed;
- secure erase claimed without executed platform/provider proof;
- client revocation represented as client-data deletion;
- tombstoned material reappears through write/import/reindex/restore;
- independent redb/Qdrant snapshots serve without paired manifest;
- restored state serves before current source/access/residency/purge/profile revalidation and new
  publication;
- missing raw output, mandatory `UNAVAILABLE` probe or self-review.

## Evidence products

Each probe binds exact command/fixture, commit/API/configuration/store/index/profile/backup/platform
identities, start/end time, `PASS | FAIL | UNAVAILABLE`, raw output digest and independent reviewer.
Prose-only evidence is rejected.

## Current disposition

```text
retention/mark/sweep implementation: ABSENT
purge implementation: ABSENT
restore quarantine/admission implementation: ABSENT
backup provider: UNSELECTED
revision-store lifecycle ports: CONTRACT_ONLY
hardening probes: UNAVAILABLE
P13 lifecycle/security: BLOCKED
```
