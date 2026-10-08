# search-candidate-validator

**C24 — Candidate Validator.**

**Status:** package boundary and agent contract only; behavior is intentionally unimplemented.

Validate nominated candidates against live security state and exact source revision/anchor readback through a vendor-neutral port.

## Owns

- security/membership/overlay checks
- exact revision/anchor verification
- stale/unreadable rejection
- replan/gap signal

## Must not own

- vendor payload evidence or client admission
- concrete revision-store/redb/Qdrant/process adapters

- **Delivery wave:** W4 / P08; hardening W7 / P13
- **Soft source-line target:** 7,500
- **Agent instructions:** [AGENTS.md](AGENTS.md)
