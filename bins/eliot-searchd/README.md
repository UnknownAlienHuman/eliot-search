# eliot-searchd

**Composition binary — the only owner of Search stores, local Qdrant process and provider server.**

**Status:** package boundary and agent contract only; behavior is intentionally unimplemented.

## Owns

- data-root owner acquisition
- concrete redb/Qdrant/process/OS adapter construction
- vendor-neutral port wiring
- bounded task supervision
- provider protocol server
- shutdown/readiness/degradation reporting

## Must not own

- capability logic
- shared clients for CLI/workers/adapters
- hidden fallback or reverse authority paths
- direct client-system canonical writes

- **Delivery wave:** W1 shell, integrated through W9
- **Soft source-line target:** 6,500
- **Agent instructions:** [AGENTS.md](AGENTS.md)
