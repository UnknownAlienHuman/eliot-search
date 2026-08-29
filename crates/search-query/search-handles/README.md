# search-handles

**Handle support for C26/C27 — Result and source handle owner.**

**Status:** package boundary and agent contract only; behavior is intentionally unimplemented.

Own opaque ephemeral and durable handles, expansion authorization, TTL/quota state and invalidation.

## Owns

- handle minting and opaque identifiers
- ephemeral handle table and durable source-handle records
- current authorization/view/revision revalidation on expansion
- TTL, binding, disclosure and invalidation state

## Must not own

- result ranking or continuation cursors
- access authority derived from a handle
- durable handles to unsaved bytes
- raw source content in handle tokens

- **Delivery wave:** W4 / P08; hardened W7 / P13
- **Soft source-line target:** 6,500
- **Agent instructions:** [AGENTS.md](AGENTS.md)
