# search-continuation

**C27 — Continuation lifecycle.**

**Status:** bounded lifecycle kernel implemented in `main`; integration and adversarial qualification remain deferred.

Own bounded opaque continuation state without exposing vendor cursors or pinning snapshots indefinitely.

## Implemented

- ephemeral in-memory candidate windows bound to process boot identity and exact epoch-pin references;
- durable immutable-data replan checkpoints with no process-local pin or unsaved bytes;
- opaque handle delivery with server-side token-digest lookup and foreign-binding nondisclosure;
- TTL, total-count, per-binding, candidate-window, issued-ID, expansion and lifecycle-batch quotas;
- exact plan/result/binding/security/view/route/profile revalidation;
- issued-candidate suppression committed only after successful emission;
- monotonic invalidation, bounded expiry, restrictive live-limit application and terminal compaction;
- explicit effects for pin renewal/release and durable-checkpoint deletion.

## Must not own

- raw Qdrant offsets or score cursors;
- silent continuation on a newer corpus;
- indefinite epoch pins;
- durable continuation containing unsaved bytes.

- **Delivery wave:** W4 / P08; hardened P13
- **Soft source-line target:** 6,000
- **Agent instructions:** [AGENTS.md](AGENTS.md)
