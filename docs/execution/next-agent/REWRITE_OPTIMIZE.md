# Next-agent task: code rewrite, ownership moves and optimization

Base: `main` at creation time. Read first: `AGENTS.md`,
`docs/handoff/AUTHORITY_MAP.md`, `docs/runtime/QDRANT_NATIVE_WINDOWS.md`,
`qualification/CURRENT_FAILURES.txt`.

## 1. T02 move-map phases 2–8 (`bins/eliot-searchd/src`, ~26k lines, limit 6500)

The reviewed move-map (scope, targets, risks, order) was accepted in
PR history — search commit messages containing `T02`. Execute strictly in
phase order, one phase per commit, green gates per phase
(`check --workspace --all-targets --all-features`, package tests,
clippy `-D warnings` honest count, `cmdkey` 0/0):

- Ph2: `control_migration*.rs` → `search-control-redb` (needs Phase 1 owner API)
- Ph3: `direct_store*` cluster → `search-revision-store` / `search-materializer`
- Ph4: `revision_protection*.rs` (contains `unsafe` DPAPI) → `search-os-secrets`
- Ph5: `source_roots*`, session files → registry/handles/continuation
- Ph6: thin service/endpoint to pure composition (keep `main` thin)
- Ph7: sealed stack → owners (recovery modules move WITH their tests)
- Ph8: delete harness snapshot/BM25 only after green product runs

Resolve O1–O9 (orphan crypto-crate registration, sealed-policy owners,
owner-lock effect home, `source_fence.rs` home, SHA-256 unification,
`snapshot_admin.rs` fate, leftover owners, budget gate, qualification lanes).
No new crates without the S31 test (real dependency/replacement/test/
context boundary). Never break `cargo test -p eliot-searchd --all-targets
--all-features` (600+ tests) or the vault-mutex hygiene.

## 2. Optimization (measured only, no invented numbers)

- Re-measure T40 ceilings on the frozen corpus after the moves; update
  `resource_budgets` ceilings only from runs, never constants.
- Kill the remaining `cargo fmt --check` drift package-by-package (never a
  workspace-wide reformat in one commit).
- `qdrant-client`/`tokio` dependency weight: keep `default-features = false`
  discipline; no new deps without boundary review.
- No behavior change may hide inside a refactor commit; no mock-as-live.

## 3. Definition of done

`bins/eliot-searchd/src` ≤ 6500 lines, workspace check/test green,
`CURRENT_FAILURES.txt` refreshed with measured numbers, every commit pushed
to `main` promptly (small commits, never lose work).
