# search-contracts

**Cell C00 — Contracts.**

Versioned external and domain types shared by every other crate.

- **Owns:** schemas, newtypes, identifiers, recipe set, reason codes, provider envelope, grant claims, execution budget.
- **Must not own:** runtime state, transport, storage, ELIOT semantics.
- **Dependencies:** none. Nothing here may reference the index client, the control journal, platform APIs or ELIOT internals.

Contract freeze delivers this crate first; every later crate compiles against it.
