# W5 current-workspace implementation packet

W5 adds truthful live-workspace observation, saved/unsaved overlay precedence and the qualified Rust
tolerant-syntax profile. It does not authorize implementation; `swarm/launch-state.toml` remains the
only launch authority.

## Packages

| Package | Operation packet | Configuration | Qualification |
|---|---|---|---|
| `search-source-reconcile` | `crates/search-source/search-source-reconcile/FUNCTIONS.md` | `config/sections/reconcile.md` | `qualification/current/W5_QUALIFICATION.md` |
| `search-overlay` | `crates/search-query/search-overlay/FUNCTIONS.md` | `config/sections/overlay.md` | `qualification/current/W5_QUALIFICATION.md` |
| `search-code-enricher` | `crates/search-prep/search-code-enricher/FUNCTIONS.md` | profile is qualification-owned, not free-form user config | `qualification/current/W5_QUALIFICATION.md` |

## Dependency-safe launch order

```text
accepted W2 source identity / registry / safe-reader / revision / unit handoffs
accepted W4 access / validator / handle / continuation contracts
    ↓
search-source-reconcile
    ├─ observation cursor/gap/inventory state
    └─ strict current-workspace preflight and live-head shadow requests

search-overlay
    ├─ saved immutable revision overlay
    ├─ authenticated unsaved memory-only overlay
    └─ exact precedence/shadow state

search-code-enricher
    └─ exact qualified no-execute Rust tolerant-syntax facts/relations/cfg predicates
```

The three package writers are independent only after every declared direct dependency/API digest and
shared current-workspace contract is accepted. Daemon integration begins after package review, not
before.

## Cross-package invariants

1. Watchers and USN are hints; inventory plus stable verified reads establish source-state continuity.
2. Overflow, reset, resume ambiguity, provider restart or root rebinding opens a gap before event
   acknowledgement.
3. Strict current-workspace requests never pass an unresolved relevant gap.
4. A partial/cancelled/timed-out inventory never closes a gap or claims complete currentness.
5. Live-head mismatch shadows/drops stale indexed nominations before evidence projection.
6. Overlay precedence is `unsaved > saved > published`, keyed by exact source/membership/generation.
7. Unsaved bytes are process-memory-only and absent from every persistent, diagnostic, backup,
   provider-cache, evaluation and learning sink.
8. Attaching/replacing an unsaved snapshot publishes its shadow atomically; older base never flashes
   through.
9. Daemon restart destroys unsaved bytes and tokens. Only saved overlays may reconstruct from immutable
   revisions.
10. Durable handles/continuations cannot target unsaved bytes.
11. Rust structure uses one exact qualified parser/grammar/profile and executes no repository build,
    proc macro, LSP/compiler build, shell, network or credential prompt.
12. Baseline structural assurance is `tolerant_syntax`, never compiler truth.
13. `cfg`/`cfg_attr` variants remain separate and unknown predicates are not unconditional.
14. Parser/profile behavior changes require re-enrichment and reprojection.
15. Vendor parser/watcher/IDE types never cross public ports.

## Hard stop conditions

- quiet watcher or elapsed time used as currentness proof;
- observation gap visible only after event acknowledgement;
- partial inventory closes a gap;
- stale base candidate emitted after live-head mismatch or overlay failure;
- unsaved content appears in any prohibited sink;
- replacement exposes a stale-base interval;
- restart reconstructs unsaved content or keeps its token valid;
- parser artifact/profile is mutable, unspecified or self-qualified;
- parser executes repository code or external tooling;
- degraded syntax facts labeled compiler-verified;
- cfg variants collapsed or ambiguity guessed away;
- missing raw output or mandatory `UNAVAILABLE` probe treated as pass.

Any stop condition keeps P09/P10 blocked. Historical retained views and DIRECT/exact source operations
may remain available when their own contracts permit it, but degraded/currentness state must be
explicit.

## Handoff requirements

Each package handoff includes:

- exact dependency commits/API digests;
- function/configuration/profile digest;
- mutable state and side-effect ownership proof;
- deterministic, negative, property and fault test outcomes;
- cancellation/deadline/crash recovery evidence;
- content-minimization and no-vendor-type proof;
- line count and split review status;
- all relevant probe IDs from `qualification/current/probes.toml`.

The integration owner accepts package receipts separately, then runs the complete W5 corpus and records
one independent P09/P10 qualification receipt.
