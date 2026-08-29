# search-handles

**Handle support for C26/C27 — Source/result handle owner.**

**Status:** package boundary and agent contract only; behavior is intentionally unimplemented.

Own opaque ephemeral and durable source handles, expansion authorization, TTL/quota state and invalidation.

## Owns

- opaque handle IDs and collision policy
- ephemeral handle table and durable source-handle records
- current authorization/view/revision revalidation on expansion
- TTL, binding, disclosure and invalidation state

## Must not own

- ranking or continuation cursors
- authority derived from handle possession
- durable handles to unsaved/unretained bytes
- raw source content or vendor IDs in tokens

- **Delivery wave:** W4 / P08; durable/security hardening W7 / P13
- **Soft source-line target:** 6,500
- **Agent instructions:** [AGENTS.md](AGENTS.md)
