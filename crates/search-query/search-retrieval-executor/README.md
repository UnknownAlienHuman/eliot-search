# search-retrieval-executor

**C23 — Bounded retrieval execution.**

**Status:** package boundary and agent contract only; behavior is intentionally unimplemented.

Execute direct, exact, indexed and optional-provider legs under bounded queues, cancellation and deterministic fusion.

## Owns

- interactive/verification/background lanes
- leg scheduling and cancellation propagation
- baseline direct/Qdrant leg dispatch
- typed extension-leg dispatch for overlay, exact and optional providers
- within-leg and cross-leg fusion orchestration
- partial-result accounting

## Must not own

- final source validation or admission
- durable query leases/history
- raw-score comparison across scoring populations
- unbounded queue, prefetch or retries
- hard dependency on later overlay, exact or optional-provider implementations

- **Delivery wave:** W4 / P08
- **Soft source-line target:** 9,500
- **Agent instructions:** [AGENTS.md](AGENTS.md)
