# eliot-search-model-worker

**Status:** binary package boundary and agent contract only; runtime behavior is intentionally unimplemented.

Host an admitted optional model provider in an isolated on-demand process.

## Owns

- worker lifecycle and IPC
- resource limits
- model-provider request dispatch
- health/cancellation reporting

## Must not own

- starting before P15+ADR
- redb/Qdrant ownership or direct access
- canonical decisions
- persistent hidden model cache

- **Delivery:** W10 / P16 after accepted P15
- **Soft source-line target:** 4,500
- **Agent instructions:** [AGENTS.md](AGENTS.md)
