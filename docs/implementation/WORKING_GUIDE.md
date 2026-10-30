# Implement, check, update

Use this page during a change, not as an extra specification. [Delivery order](README.md#delivery-sequence)
selects the next result; [code map](CODE_MAP.md) locates code; each owner recipe defines behavior.

## Start from the actual checkout

```sh
git status --short --branch
git rev-parse HEAD
```

Keep existing user changes. One manager uses one worktree and owns commits/integration. Read the registry's
next step and that stage only. Do not rebuild the audit history or wait for #598.

For a code task, name the real entrypoint, state/effect owner, immediate consumer and replaced route in a few
sentences. Then write the connected code. No ticket schema, frozen-scope form or preliminary framework is required.
A task number coordinates work, not a demand to finish every linked issue. Follow the current stage's connected
change/stop condition. Read adjacent owners only when their boundary is affected; do not preload the whole map.

## Find callers before changing a boundary

```sh
# Replace the example symbol/package with the actual one.
git grep -n -F 'put_object' -- '*.rs'
git grep -n -F 'search-revision-store' -- '*.toml' '*.rs' '*.md' '*.yml' '*.ps1'
cargo +1.98.0 metadata --locked --no-deps --format-version 1
```

Inspect Cargo target paths, module declarations, feature gates and the dispatch which reaches the function.
A file, re-export or test fixture is not a product caller. Both actual binary targets currently start in
`src/entry.rs`; their `src/main.rs` files are legacy test targets. Recheck after moving them.

Move real consumers together; delete the displaced route, exclusive fixtures and dependency edges. Do not delete
user data or shared OS/file helpers with crypto/pairing wrappers. A retained legacy path needs a concrete next
removal, not a permanent compatibility facade.

## Minimal production checks

Use the repository toolchain, existing lockfile and only the affected target/feature closure. Examples:

```sh
cargo +1.98.0 check --locked -p search-runtime-owner --lib
cargo +1.98.0 clippy --locked -p search-runtime-owner --lib -- -D warnings

# When CLI/daemon wiring changes, check the real binaries too.
cargo +1.98.0 check --locked -p eliot-search --bin eliot-search
cargo +1.98.0 clippy --locked -p eliot-search --bin eliot-search -- -D warnings
cargo +1.98.0 check --locked -p eliot-searchd --bin eliot-searchd
cargo +1.98.0 clippy --locked -p eliot-searchd --bin eliot-searchd -- -D warnings
```

Replace the example owner with changed packages and include immediate API consumers. If selected code is
feature-gated, name the smallest feature set that actually compiles it; default-only success cannot qualify
uncompiled code. Do not use `full-baseline`, `--all-features`, `--all-targets` or workspace-wide aliases by habit.
Windows-only changes need the Windows target/native check; Linux cfg exclusion is not verification of them.

Record exact command, commit, target/features and result. On failure, separate new diagnostics from existing
unrelated debt with one relevant base/head comparison. Do not suppress lints globally or repair unrelated test
harnesses. Missing toolchain/platform means NOT_RUN, not PASS. These example commands are not claimed runs.

Tests and broad native/relevance/scale runs come after the connected product. Keep later cases with their owner
and [stage checklist](TEST_CUTOVER.md); do not require a new suite or proof system before implementation.

## Documentation-only checks

```sh
git diff --check
git diff --name-status
git diff --stat
# Find incoming references before moving a page/section.
git grep -n -F 'revision-encryption.md' -- '*.md'
```

Open every changed page's relative target and named heading/HTML anchor. Check incoming links too; preserve a
meaningful anchor or update the reference. Check the short path from README/AGENTS/index to the affected owner.
For version/current-code claims, inspect the named Cargo file, lock entry or source at the recorded commit.
Compare the rule with adjacent producers/consumers; do not implement or leave two competing versions.

When removing a feature, check the touched owner's README, AGENTS, FUNCTIONS and active issue body together.
A local prohibition must not leave an adjacent instruction to construct the same feature. Preserve useful
historical evidence at an immutable revision, clearly separated from instructions. Do not use raw keyword counts
as acceptance: a warning about old encrypted data is not a request to implement encryption.

No Rust run is required for prose-only edits. Do not describe selected-page link checks as a full-repository or
external-link crawl. Do not restore old wave/receipt validators merely to make them green.

## What to update

| Changed fact | Update together |
|---|---|
| Product/security boundary | Architecture or LOCAL_SCOPE, affected owner recipes, their entrypoints and conflicting active issue bodies |
| Algorithm, errors or data lifetime | Owner FUNCTIONS.md and direct producer/consumer guidance; README links, not algorithm copies |
| File/target/package moved | Cargo/module callers, CODE_MAP row, affected links; remove obsolete facade/fixture references |
| Dependency changed | Consumer Cargo entry, Cargo.lock, donor row with version/features/source/date; distinguish inspected from executed |
| Working command now exists | Root QUICKSTART with exact executed commands, revision/platform and limits |
| Stage or unfinished work changed | Existing registry: next connected change, owner, stop condition, actual checks and unresolved limits; no second task database |

A small owner recipe answers: what enters, what it does, what leaves, what can fail, which code/callers to replace.
New settings/types appear with a real consumer, not as a future-feature catalogue. Use standard libraries and
maintained dependencies before inventing parsers, crypto, locks, process managers or storage abstractions.

## Publish

Review the complete diff, re-read the remote base/head and preserve concurrent work. Publish one coherent PR;
merge after its own checks, not after completion of the whole product. Read main/merge metadata afterwards.
Keep the registry current and short; preserve prior evidence through immutable links, not an expanding diary.
