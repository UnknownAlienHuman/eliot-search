# search-candidate-validator

**C24 — Candidate validation and readback.**

**Status:** package boundary and agent contract only; behavior is intentionally unimplemented.

Convert nominated candidates into validated source-backed evidence candidates or explicit stale/gap reasons.

## Owns

- live deny/purge checkpoint validation
- projection membership and overlay-shadow checks
- exact source revision reopen
- anchor/unit/extractor verification
- stale/unreadable rejection and replan signal

## Must not own

- emitting Qdrant payload text as evidence
- candidate-only filtering after contaminated scoring leg
- client admission decisions
- reading whatever bytes currently occupy a path

- **Delivery wave:** W4 / P08; hardened P13
- **Soft source-line target:** 8,000
- **Agent instructions:** [AGENTS.md](AGENTS.md)
