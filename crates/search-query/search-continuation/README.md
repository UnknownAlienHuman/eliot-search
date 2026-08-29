# search-continuation

**C27 — Continuation lifecycle.**

**Status:** package boundary and agent contract only; behavior is intentionally unimplemented.

Own bounded opaque continuation state without exposing vendor cursors or pinning snapshots indefinitely.

## Owns

- ephemeral in-memory continuation windows
- durable replan checkpoints
- TTL/count/binding quotas
- issued-ID suppression
- security/view/route revalidation

## Must not own

- raw Qdrant offsets or score cursors
- silent continuation on a newer corpus
- indefinite epoch pins
- durable continuation containing unsaved bytes

- **Delivery wave:** W4 / P08; hardened P13
- **Soft source-line target:** 6,000
- **Agent instructions:** [AGENTS.md](AGENTS.md)
