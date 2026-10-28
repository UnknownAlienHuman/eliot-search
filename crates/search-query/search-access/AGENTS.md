# Working on scope resolution

Read [FUNCTIONS.md](FUNCTIONS.md) and only affected query/Exact/evidence/bridge consumers.
Resolve requested/default sources against one pinned view and any real session restriction. Explicit empty stays
empty; unknown, foreign or incomplete selection fails or reports a gap and never widens.

Base population controls candidates and IDF. Request-only narrowing affects candidates, not IDF. When the base
is the entire physical collection, omit only its redundant base predicate and IDF override. Preserve request-only
candidate clauses; omit the candidate filter only when it has no clauses. Known empty scope performs no RPC.

Use current source/revision inventory for Exact and evidence. A selected restricted session owns authentication,
allowed set and revocation/cancellation of its windows, not a durable influence journal. Do not add grants,
eligibility proofs, per-membership legs or raw-score sorting across incompatible populations.

Move useful resolution into query composition if small; remove the old policy framework with real callers.
After production changes run minimal locked Rust 1.98 check/Clippy. Focused cases include whole base with nonempty
request-only narrowing, as well as empty/unknown/restricted sets and candidate/IDF parity.
