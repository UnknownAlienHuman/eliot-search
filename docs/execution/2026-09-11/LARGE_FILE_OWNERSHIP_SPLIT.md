# Large-file and ownership split

Issue: #189

Normative references:

- `docs/execution/next-agent/REWRITE_OPTIMIZE.md`
- PR #184
- `docs/execution/2026-09-05/tasks/T02.md`
- `docs/execution/2026-09-05/tasks/T10.md`
- nearest package `AGENTS.md` and `FUNCTIONS.md` for every source/destination

## Current baseline

`search-qdrant-bridge/src/real.rs` has been split into private responsibility
modules on `main`. `live.rs`, bridge `lib.rs`, several `xtask` files and daemon
storage/migration clusters remain oversized or owned by the wrong package.

## Ordered implementation

1. Split Qdrant `live.rs` into process fixture, endpoint/configuration, probe
   runner and receipt/report modules. Keep all vendor translation private.
2. Split bridge `lib.rs` into vendor-neutral contract/model/oracle modules while
   preserving one public crate entry and no second state owner.
3. Split large `xtask` modules by model/parser/rules/report concerns inside the
   same crate.
4. Move `control_migration*.rs` from `eliot-searchd` to
   `search-control-redb`/the exact persistent-state owner. The daemon retains
   only argument parsing, owner acquisition and composition.
5. Continue T02 in accepted order: direct-store cluster, secret protection,
   source roots/session state, service composition and obsolete harness code.
6. Delete old modules only after all call sites use the new owner API.

## Constraints

- one move/refactor concern per commit;
- no behavior change, persisted-format change or new acceptance claim hidden in
  a relocation commit;
- migration/recovery code moves with its tests and reason mappings;
- no forwarding-only crate or crate-per-type split;
- no searchable redb corpus, second index, dual authority, automatic migration
  or vendor leak;
- no workspace-wide formatting churn.

## Acceptance

- package source budgets and split-review thresholds are restored;
- `eliot-searchd` is a thin composition root rather than a data owner;
- every stateful operation has one package owner and no duplicate implementation;
- Qdrant upgrades remain adapter-local;
- exact tests are intentionally deferred now and later recorded as executed or
  explicitly unavailable.