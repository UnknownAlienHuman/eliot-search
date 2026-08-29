# search-qdrant-supervisor

**Process support for C01/C15 — Qualified Qdrant supervisor.**

**Status:** package boundary and agent contract only; behavior is intentionally unimplemented.

Own the exact local Qdrant artifact and Windows process lifecycle; expose no search/index data plane.

## Owns

- artifact path, digest and version qualification
- loopback binding, ACL and Job Object lifecycle
- PID/executable identity and bounded restart/quarantine
- opaque secret-reference injection

## Must not own

- collection schema, points, queries or result interpretation
- recipe/access/currentness semantics
- plaintext credentials in argv or logs
- automatic upgrades

- **Delivery wave:** W3 / P05
- **Soft source-line target:** 5,500
- **Agent instructions:** [AGENTS.md](AGENTS.md)
