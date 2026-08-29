# eliot-search-doc-worker

**Status:** binary package boundary and agent contract only; runtime behavior is intentionally unimplemented.

Host one ADR-qualified document materializer in an isolated no-execute process.

## Owns

- worker lifecycle and IPC
- provider sandbox/resource limits
- materialization request dispatch
- crash/malformed-input isolation

## Must not own

- provider selection in scaffold
- redb/Qdrant ownership or direct access
- macros, remote resources or archive execution
- Python/Node runtime without explicit ADR

- **Delivery:** W10 / P17 after accepted P15
- **Soft source-line target:** 5,000
- **Agent instructions:** [AGENTS.md](AGENTS.md)
