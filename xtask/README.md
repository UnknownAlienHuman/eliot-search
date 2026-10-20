# xtask — remove generated governance

`xtask` is currently a repository-governance product, not a small development helper. At public main
`c8e38b6` it contains 123 Rust files / 23,823 physical lines, a 3,572,618-byte canonical-digest ledger and
multiple generators/validators which freeze the historical Architecture 8.4 package/wave layout.

The current architecture does not need a replacement governance framework. Keep direct compiler/package
commands and real behavior checks; delete generated authority and source-shape policy.

## Current command surface and disposition

| Command family | Current behavior | Disposition |
|---|---|---|
| accepted-evidence digest | Canonicalizes package handoff evidence into another reviewed digest/profile and parity corpus. | Delete. Git/PR history and direct result artifacts are enough; no accepted-package authority remains. |
| architecture coverage/contracts | Parses hundreds of Markdown headings and registry relations into an exact package/module ownership graph. | Delete. Architecture is a manifesto; implementation docs and actual Cargo/callers own current direction. |
| coverage graph / package maps | Generates and validates committed package bundles and relation maps. | Delete generators, validators, 180 generated TOML outputs and their workflows. |
| canonical digest guard | Scans source and requires 5,960 sites to match a 72,002-line approval ledger. | Delete ledger, scanner/allowlist/review schema and merge gate under #525. Keep actual digest/preimage tests. |
| Qdrant boundary | Walks the repository, partially parses Rust/import aliases and cross-checks frozen qualification/version records. | Replace with Cargo metadata/dependency review, private bridge API and connected live behavior. Delete partial Rust analyser. |
| integration bootstrap | Freezes exactly 45 members, five wave profiles, exact aliases, seven directories, `CONTROL_JOURNAL_ONLY`, migration quarantine and an old workflow. | Delete. Cargo/rust-toolchain/lockfile and current root manifest are their own source of truth. |

Historical reports may remain in Git history. Do not create an archive copy, compatibility wrapper, new map
format or smaller approval database.

## Measured generated state

The already audited inventory is:

```text
xtask Rust                                      123 files / 23,823 lines
coverage/architecture/package-map Rust           49 files / 9,090 lines
swarm/coverage                                  205 TOML / 152,921 lines
generated package-maps subset                   180 TOML / 78,804 lines / 2,588,784 bytes
canonical-digest-ledger.json                    5,960 sites / 833 paths / 72,002 lines / 3,572,618 bytes
Qdrant partial Rust source analyser subtree       16 files / 3,293 lines including tests
```

These are gross deletion candidates, not promised net deletion. Shared Cargo metadata helpers survive only
when an actual remaining command consumes them.

## Direct development tools

During the refactor, use the smallest command which checks the changed production owners and immediate
consumers:

```text
cargo +1.98.0 check --locked -p <changed-package> [other actual consumers]
cargo +1.98.0 clippy --locked -p <changed-package> [other consumers] -- -D warnings
```

Run focused behavior/live cases when the connected feature exists. Full workspace/all-features/all-tests are
late integration/release operations, not a prerequisite for every deletion slice.

Useful repository facts require no custom authority:

- `rust-toolchain.toml` selects the toolchain;
- `Cargo.toml` is the workspace/package source of truth;
- `Cargo.lock` is checked by Cargo `--locked` and an optional direct regeneration diff;
- `cargo metadata --locked --format-version 1` supplies the dependency graph;
- Rust visibility/compiler errors protect vendor/private API boundaries;
- `git diff --check` and focused link checks protect edited documentation;
- real CLI/Qdrant/source tests establish behavior.

If no substantial custom command remains after removal, delete the `xtask` package entirely. Do not retain a
crate for usage text, JSON reports or wrappers around direct Cargo commands.

## Workflows and wrappers

Delete together with their last command/data consumer:

- package-map/coverage/architecture workflows and PowerShell wrappers;
- accepted-evidence digest workflow/wrappers/profiles/fixtures;
- integration-bootstrap validator/workflow and frozen build/layout registries;
- `refactor-boundaries.yml` module-ownership/source-shape matrix;
- swarm/stage/function/optional-wave validators which only preserve retired architecture;
- qualification directories whose PASS means only structural conformance to those registries.

A small manual workflow may remain for an explicitly requested compiler/Clippy or later release suite, but it
runs direct commands and records raw outcomes. It does not issue acceptance, freeze module layout or require
regeneration of retired artifacts.

## Qdrant boundary after deletion

Keep `qdrant-client` as a private direct dependency of the selected bridge. Verify with:

1. `cargo metadata`/manifest review that no other production package directly depends on it;
2. compile-time private types and a direct public-API review;
3. focused real bridge/publication/query execution.

Do not implement another Rust parser, rustdoc analyser, source allowlist, qualification-token comparison or
vendor-type taint engine.

## Completion

Close #214/#525 when retired commands, wrappers, workflows, registries, generated outputs and tests are gone;
Cargo metadata/direct compiler checks still work; and normal implementation no longer requires map, ledger,
status, issue-state, module-shape or review-badge updates. Report actual code/data deletion separately.
