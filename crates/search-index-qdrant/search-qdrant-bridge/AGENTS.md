# Working on the Qdrant bridge

Read [FUNCTIONS.md](FUNCTIONS.md) and the affected [publication recipe](../search-publication/FUNCTIONS.md).
Use one daemon-owned pinned client/runtime; vendor types and credentials remain private.

Implement only operations used by the current stage. Stage 2/3A builds complete vectorless scoring-text
generations and queries them through TextQuery. No sparse inference, point closure, sequence or local BM25.
Stage 3B mutation is conditional on failed measured budgets; physical point deletion is absent in both profiles.

Use the same fixed inputs and absolute execution context for an attempt. An ambiguous external effect is not
success or proof of cancellation. Publication owns recovery; the bridge must not create another mutation ledger.
Known empty scope performs no RPC; whole base population does not remove request-only candidate filters.

Remove obsolete supervisor/qualification/lease wrappers with callers. Run minimal locked Rust 1.98 check/Clippy
for bridge and actual publication/query consumers after code; record native behavior separately.
