# Repository development tools

Use direct Cargo/Git commands and focused behavior checks. The historical structural-validation system is
being removed under [#214](https://github.com/UnknownAlienHuman/eliot-search/issues/214) and
[#525](https://github.com/UnknownAlienHuman/eliot-search/issues/525); see [`xtask/README.md`](../xtask/README.md).

## During implementation

Check only changed production owners and their real immediate consumers:

```powershell
cargo +1.98.0 check --locked -p <package> [-p <consumer> ...]
cargo +1.98.0 clippy --locked -p <package> [-p <consumer> ...] -- -D warnings
git diff --check
```

Run focused cases for the behavior being connected. Native Windows, live Qdrant, restart, scale and broad
workspace claims require actual execution after the corresponding product slice exists. Full workspace/
all-feature/all-test matrices are late integration or release operations, not mandatory proof for every
refactor commit.

## Retired tooling

Remove these families with their last consumers:

- architecture-coverage and architecture-contract registries;
- coverage graph and committed package-map generation/validation;
- accepted-evidence digest profiles and wrappers;
- canonical per-site digest ledger/approval guard;
- integration-bootstrap frozen package/profile/layout validator;
- Qdrant partial Rust source analyser and qualification-token comparison;
- function/stage/swarm/wave packet validators;
- source/module/layout/line-count ownership tests which preserve deleted implementations.

Do not regenerate their outputs, preserve their old command names or replace them with another map, registry,
approval service or formal review badge. Historical records remain in Git.

## Facts available directly

- `rust-toolchain.toml` selects the Rust version and components.
- `Cargo.toml` and `cargo metadata --locked --format-version 1` describe the current workspace/dependencies.
- Cargo `--locked` and an optional `cargo generate-lockfile` diff check the lockfile.
- Rust privacy and compiler errors protect private vendor types.
- Direct source/API review and live tests protect Qdrant and platform boundaries.
- `git diff --check` and a lightweight link scan protect edited documentation.

A small manual workflow may run one of these commands and record raw outcomes. It must not issue package,
gate, wave, acceptance or readiness authority.

## PowerShell

Delete wrappers whose only purpose is invoking retired xtask/registry validators. Keep a PowerShell script
only when it performs a real Windows effect or materially improves a current operator workflow that cannot be
expressed directly. A wrapper around one Cargo command is not a permanent subsystem.
