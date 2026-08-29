# eliot-searchd

**Status:** binary package boundary and agent contract only; runtime behavior is intentionally unimplemented.

Compose the capability crates, own the data root and expose the only storage/provider process boundary.

## Owns

- dependency wiring and startup order
- data-root owner guard
- bounded task supervision
- provider protocol server
- controlled shutdown and readiness/degradation reporting

## Must not own

- reimplementing capability logic inside main
- sharing store clients with CLI/workers/adapters
- allowing another process to own qdrant or redb
- hidden fallback across capability boundaries

- **Delivery:** W1 shell, integrated through W9
- **Soft source-line target:** 6,500
- **Agent instructions:** [AGENTS.md](AGENTS.md)
