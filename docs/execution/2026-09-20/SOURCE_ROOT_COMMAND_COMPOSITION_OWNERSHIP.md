# PR #193: source-root command composition ownership

Date: 2026-09-20
Tracking: issue #189 / PR #193 / T02 service-composition phase

## Ownership correction

The four source-root CLI commands were implemented in the daemon-root module
`source_root_commands.rs`, but their only caller was the top-level app dispatcher.
That left a command-family owner outside the command application that routes,
reports and maps its failures.

The unchanged implementation now lives at:

```text
app/kernel/source_root_commands.rs
```

`app/kernel.rs` declares the private module, and sibling `dispatch.rs` invokes it
through `super::source_root_commands`. The primary binary entry no longer declares
a separate root-level command owner, and the obsolete file is deleted.

## Preserved behavior

This is a byte-preserving relocation of the command implementation. The following
remain unchanged:

- argument-count and command-name validation;
- one `DataRootGuard` for the whole operation;
- source-root register/unregister JSON;
- catalog views and explicit observation-gap reporting;
- current-workspace projection and reason selection;
- preflight refusal when any root is missing or unavailable;
- sequential multi-root sync accounting;
- conservative `effects_may_have_committed=true` on interrupted sync;
- sync-proof marking only after every root completes;
- Windows path and control-character JSON escaping;
- existing reason codes and process exit mapping.

The module continues to compose existing owners only. It does not acquire source
registry, filesystem identity, revision-store, currentness or access authority.

## Boundary decision

The command family remains daemon app composition because it owns stdout JSON,
argument routing, a data-root owner guard and orchestration across root catalog,
DIRECT store and directory synchronization. Moving it into
`search-source-registry` would give a pure registry package process, filesystem
and output responsibilities explicitly forbidden by its contract.

## Regression coverage

`app_module_ownership` now requires:

- the command family under `app/kernel`;
- dispatcher calls through its sibling module;
- no root-level module declaration;
- permanent absence of `src/source_root_commands.rs`;
- continued separation from top-level `run_main` and provider transports.

The existing command-local tests remain attached to the relocated module and
continue to cover JSON escaping and argument rejection before data-root opening.

## Required execution

```text
cargo test --locked -p eliot-searchd app::kernel::source_root_commands::tests
cargo test --locked -p eliot-searchd --test app_module_ownership
cargo check --locked -p eliot-searchd --all-targets --all-features
cargo fmt --all -- --check
cargo clippy --locked -p eliot-searchd --all-targets --all-features \
  -- -D warnings
```

Execution status in the current authoring environment: **NOT_RUN** (`cargo`,
`rustc`, and `rustfmt` are absent). No runtime qualification, T02 acceptance or
independent review is claimed.
