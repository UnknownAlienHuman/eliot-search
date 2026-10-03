# search-retrieval-executor

**C23 — bounded retrieval execution and deterministic fusion.**

**Status:** substantive source exists for plan admission, quotas, cancellation, epoch/route pins, direct and
indexed nomination execution, contamination checks, deterministic fusion and partial coverage. It owns no
concrete Qdrant or source-read adapter and is not yet installed as a complete public live recipe path.

## Owns

- bounded admission, scheduling and binding quotas;
- typed direct/indexed/provider leg dispatch through vendor-neutral ports;
- cancellation/deadline propagation;
- route/epoch pin lifetime;
- population/fence validation;
- whole-leg contamination decisions;
- versioned deterministic rank fusion;
- execution and partial-coverage accounting.

## Must not own

- final source-backed candidate validation or result admission;
- durable ordinary-query history;
- concrete Qdrant/redb/filesystem/process clients;
- raw score comparison across inequivalent access/scoring populations;
- silent indexed-to-DIRECT success substitution;
- unbounded queues or hidden retries.

## Current gaps

The indexed kernel can call an accepted `IndexedRetrievalPort`, but daemon composition still documents a
process-test/oracle adapter rather than the final live Qdrant adapter. Completion requires the aligned
Qdrant stack, authoritative candidate readback, public recipe wiring and real end-to-end qualification.
Free-text/orientation and its baseline ranking profile are tracked by #213 and #221.

This package is `SOURCE`, not `QUALIFIED` or publicly `ENABLED`.

- **Product area:** Architecture S19/S21/S30
- **Agent instructions:** [AGENTS.md](AGENTS.md)
- **Function contract:** [FUNCTIONS.md](FUNCTIONS.md)
- **Current status matrix:** [../../../docs/product/IMPLEMENTATION_STATUS.md](../../../docs/product/IMPLEMENTATION_STATUS.md)
