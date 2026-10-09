# search-qdrant-supervisor

**Process support for C01/C15 — Qualified local Qdrant supervisor.**

**Status:** `SOURCE_PRESENT`; artifact, process-identity, restart/quarantine and secret-bound lifecycle source exists, while final Windows/native runtime qualification and daemon composition remain incomplete. See [central package status](../../../docs/product/PACKAGE_STATUS.toml). Current owner: #120.

Own exact artifact qualification and Windows process lifecycle; expose no search/index data plane.

## Owns

- executable version/digest/path and process identity qualification
- loopback bind, ACL and Job Object lifecycle
- bounded restart, quarantine and shutdown
- opaque API-secret reference consumption

## Must not own

- collection schema, points, queries or publication semantics
- plaintext credential storage
- automatic upgrades
- client/CLI direct access

- **Delivery wave:** W3 / P05
- **Soft source-line target:** 5,500
- **Agent instructions:** [AGENTS.md](AGENTS.md)
