# eliot-searchd

**Composition binary — sole owner of Search stores, local Qdrant process and provider server.**

**Status:** package boundary and agent contract only; behavior is intentionally unimplemented.

## Owns

- data-root owner acquisition and progressive startup
- concrete redb/OS-secret/Qdrant adapter construction
- vendor-neutral port wiring
- bounded request supervision and provider server
- shutdown/readiness/degradation reporting

## Must not own

- capability logic
- shared clients for CLI/workers/adapters
- reverse adapter edges into query/lifecycle packages
- hidden fallback or client canonical writes

- **Delivery wave:** W1 shell, integrated progressively through W9
- **Soft source-line target:** 6,500
- **Agent instructions:** [AGENTS.md](AGENTS.md)
