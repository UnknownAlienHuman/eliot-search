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

## Implementation choices

Optimize the number of state owners, representations and call hops, not the line count. Start with the real
caller and a private function or concrete struct; add a boundary only when it does identifiable work.

| Decision | Default | Add more only when |
|---|---|---|
| Function/module | A readable operation with private helpers | A helper names coherent behavior, limits a borrow/transaction lifetime or removes actual repetition; not one file per verb |
| Crate | Use the existing consuming owner | A real dependency, platform or substantial reusable algorithm needs isolation that a module cannot provide; a new noun is not a package |
| Trait/adapter | Concrete call or an existing library trait | Actual implementation variation or a dependency cycle needs a narrow local interface; not a trait around every owner |
| Value/type | Reuse a value with the same meaning; move/borrow it | A distinct invariant, ownership lifetime or wire/storage boundary requires another type; a newtype should prevent a named mistake |
| State/task | One mutable owner; ordinary calls for pure steps | Work must run concurrently for responsiveness or a real resource boundary; reuse the existing runtime and bound ownership/queues |
| Dependency | Existing maintained mechanism after reading its API | It replaces owned complexity and fits the actual consumer; no universal wrapper over every donor method |

A concrete vendor adapter can hide SDK types without a matching trait. Standard traits and small local test seams
are not a global ports catalogue. Do not invent a second backend or test framework to justify an interface, or
force unrelated semantics through one generic helper just to remove similar lines.
Use private visibility by default. Arc shares genuinely shared ownership; a mutex protects actual mutable sharing,
not a borrow-checker workaround. Preserve the existing view/window lifetime and no-lock-across-I/O rules.

A change does not need Manager → Service → Repository → Port → Adapter layers. Each retained layer must own
behavior, a lifetime or a real dependency conversion; a forwarding rename is not a boundary. Do not enforce
function/file-size quotas, create trait/impl pairs for every struct, or compress the real algorithm into clever
one-liners. Keep named helpers where they make the flow easier to inspect.

## Data, failures and work

Decode/validate at external or persisted boundaries, then carry the checked value to the real consumer. Do not
serialize/hash/reparse an in-process handoff or clone a complete catalogue to change one row. Move acquired bytes
through retention/preparation; use references or bounded pages where that suffices. Persistence/restart and actual
object-integrity checks still require their owner's validation: removing duplicate checks must not remove those.

Wire and storage models may differ for a real representation or validation reason. Do not build Raw/Parsed/Plan/
Permit/Receipt families that merely copy the same fields. [Shared data](../../crates/search-contracts/FUNCTIONS.md)
owns what is shared; [effect owners](README.md#owners) own their state and errors.

Represent errors the caller actually handles. Preserve the cause until the public edge maps it once to a bounded,
redacted error/gap. No generic error registry, one enum per helper or string matching for control flow. Missing,
corrupt, cancelled and not-yet-ready are not interchangeable empty successes. Do not hide a failed operation with
unwrap_or_default, a blanket catch or a silent legacy fallback. Retry only where the actual owner knows it is safe;
a timeout is not rollback. Do not add a universal retry/compensation service.

Bound real bytes, allocations, queue entries and work before expensive operations. Reuse the original deadline
and charged counters. Never add a whole-corpus scan, per-item full-object verification, global sort or duplicate
body copy just to produce status/receipts. Share required integrity reads where safe; measure necessary scans
against the selected corpus budgets before adding caches, incremental histories or parallel workers.

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

A small owner recipe answers: input → real operation → output, failure behavior, source/caller cutover and checks.
Document invariants and chosen mechanisms, not a class diagram to instantiate verbatim. Keep examples at the
smallest current stage; future handlers/settings do not get placeholder DTOs. README routes to the recipe and
AGENTS gives editing constraints; neither copies its algorithm or a full historical caller census.

## Review the causal path

Before publishing, follow one real input through dispatch, state/effect and output. Check each new abstraction's
specific job, each copy/scan's cost, and each durable row's restart or user-data need. Remove forwarding-only
hops, duplicated representations and unreachable fallback routes. Verify that cancellation, conflict and missing
state keep their meaning and that a partial result cannot become complete success.

The old consumer must reach the replacement, not an uncalled helper beside the old path. Search imports, module/
feature wiring and actual dispatch again for the displaced route. A needed temporary dependency keeps a concrete
next removal; do not finish every legacy owner before landing the current connected change.

These are review questions, not a new form, score, quota or gate framework. A smaller diff with a lost invariant
is not an improvement. Do not use fewer lines or passing Clippy alone as evidence of correct behavior.

## Publish

Review the complete diff, re-read the remote base/head and preserve concurrent work. Publish one coherent PR;
merge after its own checks, not after completion of the whole product. Read main/merge metadata afterwards.
Keep the registry current and short; preserve prior evidence through immutable links, not an expanding diary.
