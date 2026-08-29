# W5 current-workspace settings 1.0

Machine schema: [`../../config/w5-current.toml`](../../config/w5-current.toml).

## Design rule

Configuration may tune finite observation, transient-memory and parser resource limits. It cannot
weaken source truth, currentness, non-persistence, shadowing or syntax-assurance invariants.

Modes:

- `LOCKED` — compiled correctness/security invariant; every override rejected;
- `TUNABLE` — bounded ordinary owner setting;
- `TUNABLE_INTERNAL_CEILING` — integration/host/profile policy, not a new public section;
- `QUALIFIED_REF` — immutable accepted dependency/profile/evidence reference.

`search-config` handles layering/provenance/diff only. Semantic owners remain:

```text
search-source-reconcile   reconciliation + currentness
search-overlay            saved/unsaved overlay + shadow lifecycle
search-code-enricher      qualified Rust syntax profile + limits
```

Ordinary user sections are the existing `reconcile` and `overlay` sections. Currentness and Rust parser
invariants/qualified refs are internal W5 policy until explicitly accepted; they cannot be introduced as
arbitrary new top-level TOML sections.

## Reconciliation

Locked:

```text
watcher is a hint, never authority
watcher overflow/cursor loss declares gap before acknowledgement
open gap blocks current_confirmed
complete guarded inventory required to resolve gap
partial inventory cannot remove unseen prior sources
registry/root/admission/cursor guards required at apply
```

Inventory interval, root/slice/entry/retry/background limits are finite. Increasing interval does not
permit a stale scope to remain `current_confirmed`. Reducing slice/budget may pause/replan work and keeps
the gap open until complete.

## Currentness

Watcher events and Qdrant health cannot confirm filesystem currentness. Unsaved buffer currentness is a
separate binding/snapshot axis and cannot upgrade disk/workspace currentness. Filesystem, saved revision,
buffer snapshot and projection currentness remain separately reported.

These are internal locked rules with no user override.

## Overlay

Locked:

```text
unsaved bytes/units/vectors never persist to redb, CAS, Qdrant, logs or backups
unsaved visibility is binding/workspace scoped
shadow fence applies before base retrieval and IDF
post-candidate dedup cannot repair missing shadowing
overlay is not a second durable database
durable handle cannot target unsaved buffer
```

Tunable source/byte/entry/unit/feature/candidate/TTL quotas are finite. Restrictive reduction:

1. blocks new excess admission;
2. deterministically identifies excess/expired entries;
3. publishes new overlay/shadow revision and invalidation receipts before acknowledgement;
4. releases unsaved bytes/local state from memory;
5. does not delete immutable source revisions or claim secure erase.

Profile/source-view identity changes require reprepare/replan, not reinterpretation.

## Rust enrichment

`parser_profile_ref` is `UNSELECTED` until one exact parser/grammar/dependency/fixture profile is accepted.
It is not a free-form crate/version setting.

Locked false:

```text
Cargo execution
rustc execution
build scripts
macro expansion
network/package resolution
compiler-semantic certainty from syntax
```

Input/node/depth/error/fact ceilings are finite internal profile values. Changing any profile or bound
affects output completeness/failure semantics and requires re-enrichment plus projection rebuild. It
cannot apply live to existing facts.

## Layering and publication

Layering remains `compiled defaults < file < captured environment < captured CLI` only for fields the
owner's existing section explicitly exposes. A higher layer cannot unlock a locked/internal/qualified
field.

The prior effective config remains authoritative until every required pause/invalidation/reprepare/
rebuild receipt succeeds. Failed change leaves explicit pending/rejected state and never publishes mixed
old/new currentness, shadow or parser-profile semantics.

## Redaction

Default diagnostics may contain section/field, provenance layer, validation reason, bounded counts and
content-free identity digests. They exclude source/unsaved bytes, parser input, lexical vectors, exact
paths, secrets and inaccessible source names.

## Required settings tests

- one owner/type/mode/default and finite bounds per field;
- locked/internal/qualified fields reject unauthorized layers;
- watcher/gap/currentness invariants cannot be disabled;
- partial inventory cannot remove/resolve currentness;
- unsaved non-persistence/cross-binding/shadow-before-IDF locked;
- all overlay quotas finite and restrictive reductions emit invalidation;
- parser profile absent/unqualified rejects enrichment;
- Cargo/rustc/build/macro/network/semantic-overclaim locked false;
- parser limits finite and identity-affecting changes require rebuild;
- failed composite change preserves prior authoritative config fingerprint;
- redacted diagnostics contain no source/buffer/secret/path content.
