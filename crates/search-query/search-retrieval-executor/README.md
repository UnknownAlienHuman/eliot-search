# search-retrieval-executor

**C23 — Retrieval Executor.**

**Status:** `SOURCE_PRESENT`; bounded lane execution, cancellation, typed dispatch and fusion source exists, while accepted ranking profiles and real bridge/provider product composition remain incomplete. See [central package status](../../../docs/product/PACKAGE_STATUS.toml). Current owners: #125/#127/#200.

Execute bounded direct/index/provider legs through vendor-neutral ports and fuse only compatible authorized outputs.

## Owns

- execution lanes, scheduling and cancellation
- typed leg dispatch
- deterministic rank fusion
- partial coverage accounting

## Must not own

- final validation/admission
- durable query history
- concrete Qdrant/redb/process adapters

- **Delivery wave:** W4 / P08
- **Soft source-line target:** 7,500
- **Agent instructions:** [AGENTS.md](AGENTS.md)
