# W7 lifecycle, purge and restore implementation packet

W7/P13 hardens restrictive security, handles/continuations/candidate emission, Search-owned CAS lifecycle,
purge and restore. It does not authorize implementation; `swarm/launch-state.toml` remains the sole
launch authority.

## Package packets

| Package | Packet | Role |
|---|---|---|
| `search-retention` | `crates/search-runtime/search-retention/FUNCTIONS.md` | root/mark/sweep, purge/tombstone, paired recovery and restore admission owner |
| `search-revision-store` | `crates/search-source/search-revision-store/FUNCTIONS.md` | immutable objects/revisions/leases and exact lifecycle object operations |
| `search-access` | `crates/search-query/search-access/W7_HARDENING.md` | monotonic restrictive fence, checkpoints and invalidation completion |
| `search-handles` | `crates/search-query/search-handles/W7_HARDENING.md` | durable eligibility, invalidation and token non-resurrection |
| `search-continuation` | `crates/search-query/search-continuation/W7_HARDENING.md` | checkpoint/pin invalidation and restore non-reuse |
| `search-candidate-validator` | `crates/search-query/search-candidate-validator/W7_HARDENING.md` | source-backed lifecycle checkpoints and restored/purged rejection |
| `search-publication` | `crates/search-index-qdrant/search-publication/W7_HARDENING.md` | invalidation-only commit and purge/restore publication interaction |
| `search-index-reclaimer` | `crates/search-index-qdrant/search-index-reclaimer/W7_HARDENING.md` | ordinary reclaim boundary and receipt truthfulness |

Machine evidence: [`../../qualification/lifecycle/`](../../qualification/lifecycle/README.md).  
Machine settings: [`../../config/w7-lifecycle.toml`](../../config/w7-lifecycle.toml).

## Dependency-safe launch order

```text
accepted W0–W6 contracts and receipts
        ↓
search-revision-store lifecycle ports
search-access restrictive fence/checkpoints
search-handles / search-continuation / search-candidate-validator hardening
search-publication invalidation/purge/restore hardening
search-index-reclaimer boundary hardening
        ↓
search-retention root/mark/sweep and invalidation orchestration
        ↓
purge fence + exact layer receipts
        ↓
paired recovery / restore quarantine / revalidation / new publication
```

The retention writer consumes accepted public ports and hardening receipts; it does not edit or
reimplement another package's state. Independent hardening writers may run in parallel only after exact
direct handoffs/tickets are accepted.

## Cross-package invariants

1. Restrictive control state is monotonic and acknowledged only after live immutable fence publication.
2. Missing/failed dependent invalidation receipt keeps affected scope fail-closed.
3. Restrictive changes are checked at every request/leg/readback/validation/handle/continuation/exact/
   restore emission checkpoint.
4. Population contamination discards the whole affected scoring/IDF leg, not one candidate.
5. Handle possession never grants access; durable handles require exact retained immutable revisions.
6. Durable handle/continuation state never targets unsaved bytes; ephemeral state never survives restart.
7. Restored bearer tokens/checkpoints remain quarantined and are not automatically serving-valid.
8. Candidate validation rejects physically present/restored material under purge/old owner/old epoch.
9. Retention root set includes every architecture-required durable root plus fresh active-pin protection.
10. Reachability is manifest-graph based; reference count alone never authorizes deletion.
11. Mark/sweep uses one frozen root/control/policy/pin/inventory generation and exact object IDs.
12. Partial/cancelled mark, stale roots/pins or unreadable manifest cannot authorize sweep.
13. Live purge fence and tombstone precede acknowledgement and destructive work.
14. Failure of later purge layers never reopens logical access.
15. Handle/continuation/index/cache/CAS/backup/client layers retain separate owner receipts/statuses.
16. Ordinary point reclaim and CAS sweep cannot satisfy security/legal purge.
17. Shared physical object may remain for another root while purged scope stays logically inaccessible.
18. Search never claims authority to delete client-owned canonical evidence.
19. Search reports secure-erasure limitation unless exact provider/platform proof exists.
20. Tombstone blocks write/import/reindex/remint/restore resurrection.
21. Recovery manifest pairs redb, Qdrant and object roots with publication/purge/schema/profile identities.
22. Independent or mismatched store snapshots remain quarantined/non-serving.
23. Restore revalidates current source identity/owner/membership/access/residency/revision/profile/tombstone.
24. Old backup epoch/route is never restored directly as current.
25. Serving after restore requires exact rebuild/readback and one new guarded publication/route commit.
26. Restore cancellation/crash remains quarantined.
27. Vendor types, broad correctness deletes and source content never cross public lifecycle ports.

## Hard stop conditions

- restriction acknowledged before live fence;
- revocation/purge race passes a mandatory checkpoint;
- contaminated scores/counts/traces survive candidate-only filtering;
- durable handle/checkpoint targets unsaved or unretained content;
- token existence leaks across bindings;
- missing root/pin/manifest state permits sweep;
- refcount-only or broad-prefix deletion path appears;
- partial/cancelled mark authorizes deletion;
- ordinary reclaim/sweep receipt accepted as purge;
- later purge layer failure removes live deny;
- secure erase or client-data deletion overclaimed;
- tombstoned material reappears through write/import/reindex/remint/restore;
- unpaired redb/Qdrant restore serves;
- restore serves before revalidation/new publication;
- mandatory probe remains `UNAVAILABLE`, raw evidence is absent or producer self-reviews.

Any hard stop keeps P13 blocked.

## Handoff requirements

Each package handoff records:

- exact ticket/base/direct dependency commits and API/configuration digests;
- owned lifecycle state and operation/receipt classes;
- idempotency, cancellation and unknown-outcome readback behavior;
- every applicable revocation/purge checkpoint;
- exact root/manifest/object/route/source/profile/tombstone identities;
- deterministic/negative/property/fault/security test results;
- non-content diagnostics/leakage audit;
- applicable probe IDs from `qualification/lifecycle/probes.toml`;
- line count/split-review state and unresolved provider/platform evidence.

The integration owner accepts each package independently, then executes the complete 60-probe corpus
before issuing the P13/G3 lifecycle receipt. Compilation and structural CI alone do not pass W7.
