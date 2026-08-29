# W5 current workspace, overlay and Rust structure qualification contract

**Status:** `NOT_EXECUTED`  
**Architecture:** ELIOT Search 8.4, S16–S18, S23–S24, H3, H6, H13–H16, P09–P10  
**Scope:** truthful observation continuity, bounded reconciliation, saved/unsaved overlay precedence,
unsaved-byte non-persistence, live-head shadowing and qualified Rust tolerant-syntax facts.

A quiet watcher, successful demo edit, parser compilation or unit-test collection is not qualification.
Every mandatory probe in [`probes.toml`](probes.toml) must execute against exact accepted package API
digests, configuration fingerprint, platform/provider identities and immutable fixture digests.

## Owners

| Evidence | Owner |
|---|---|
| cursor/gap/inventory/currentness/reconcile recovery | `search-source-reconcile` |
| stable no-execute change verification | `search-safe-reader` |
| source identity/path-binding interpretation | `search-source-identity` |
| saved/unsaved state, precedence, shadows and direct candidates | `search-overlay` |
| authenticated IDE buffer session feed | daemon/IDE adapter composition, not overlay core |
| Rust parser profile, facts, relations, cfg predicates and assurance | `search-code-enricher` |
| exact revision/anchor readback | `search-revision-store` |
| query-time stale rejection and live security check | `search-candidate-validator` |
| content-minimized leakage/evidence aggregation | `search-eval` |
| end-to-end startup/resume/shutdown/degradation | `eliot-searchd` |

One package cannot accept its own evidence. The integration owner binds exact commits/API/configuration/
fixture/provider identities and an independent reviewer receipt.

## Frozen inputs

Before execution, publish immutable identities for:

- repository commit, Rust toolchain and `Cargo.lock`;
- accepted contracts/domain/ports/config/source/query API digests;
- effective configuration and section fingerprints;
- root/source registry, owner epoch and observation provider identities;
- Windows watcher/USN adapter build and test-root descriptors;
- IDE/buffer adapter protocol, pairing and authorization fixture identities;
- exact Rust parser/grammar/query implementation, version/source checksum/license;
- source/materialization/unit/coordinate fixture manifests;
- control corpus and probe registry digests;
- platform, resource envelope and crash-test driver identity.

Mutable branches, unspecified parser defaults and locally edited fixtures are invalid inputs.

## Execution order

1. Start from an admitted root and a deliberately stale prior inventory.
2. Exercise startup reconciliation before watcher admission.
3. Execute contiguous, duplicate, out-of-order, overflow, reset, resume and adapter-restart cursor cases.
4. Run bounded multi-slice inventory, cancellation/deadline and crash-after-control-commit cases.
5. Prove strict current-workspace preflight blocks every unresolved relevant gap.
6. Change live source heads between indexed nomination and validation; verify immediate shadow/drop/reconcile.
7. Attach authenticated unsaved snapshots and exercise replacement, close, disconnect, expiry, revocation,
   purge, owner change and daemon restart.
8. Audit every prohibited unsaved persistence/disclosure sink.
9. Exercise saved-overlay recovery and explicit unsaved→saved revision admission.
10. Run the exact qualified Rust parser profile over valid, malformed, cfg-heavy, macro and coordinate
    fixtures under resource/cancellation limits.
11. Validate structural facts/relations/roles/assurance/anchors and deterministic manifest bytes.
12. Publish raw outputs plus independent review. Only then may P09–P10 evidence be accepted.

## Mandatory properties

### Observation and currentness

- watchers and USN records are hints, never source truth;
- startup/resume/periodic/explicit/overflow reconciliation paths exist;
- overflow, journal reset/wrap, provider restart, root rebind and unprovable cursor ordering open a gap;
- a gap is observable by query preflight before the triggering event is acknowledged;
- partial/cancelled/timed-out inventory cannot close a gap or advance complete inventory revision;
- `CURRENT_CONFIRMED` requires continuous relevant cursor plus completed verified inventory;
- relaxed observed mode exposes freshness state and age and is unavailable to exact negative proof;
- historical/frozen retained revisions remain distinguishable from live current workspace;
- live-head mismatch shadows/drops the indexed nomination before evidence emission;
- control commit uncertainty is resolved by operation/readback, not duplicate commit or newer inventory.

### Overlay

- saved overlay points only to admitted immutable revisions;
- unsaved bytes exist only in guarded process memory and are authenticated/binding/snapshot/TTL/quota
  scoped;
- unsaved bytes never enter redb, CAS, Qdrant, logs, metrics, traces, backups, restore manifests, crash
  attachments, provider caches, evaluation corpora or learning/training inputs;
- precedence is `unsaved > saved > published`, keyed by exact source/membership/generation identity;
- shadow state is installed atomically with attach/replacement so stale base never flashes through;
- close/replacement/disconnect/expiry/revocation/purge/owner change invalidates immediately;
- budget/provider failure preserves shadow and reports a gap rather than exposing stale base;
- daemon restart destroys unsaved bytes/tokens; only saved overlays can reconstruct;
- a durable source handle/continuation cannot target unsaved content;
- explicit save transition requires a matching admitted durable `SourceRevision` receipt.

### Rust structural enrichment

- one exact parser/grammar/query/profile identity is qualified; `latest`/ranges/floating revisions are
  rejected;
- parser runs in-process or an explicitly accepted isolated profile without executing repository code;
- build scripts, proc-macro expansion, shell/network, credential prompts and LSP/compiler build commands
  are absent;
- every fact/relation carries source revision, representation/unit, native anchor, profile digest,
  tolerant-syntax assurance, evidence role and configuration predicate;
- malformed/recovery parses are bounded and marked degraded; they never claim compiler truth;
- `cfg`/`cfg_attr` all/any/not/key/value variants are preserved and not evaluated without explicit
  target/feature context;
- unresolved name/call/trait relations stay descriptive/ambiguous;
- coordinate or digest mismatch rejects or lowers assurance explicitly;
- cancellation/resource limits yield gaps, never a falsely complete manifest;
- parser/profile behavior change requires re-enrichment and reprojection.

## Stop conditions

Any of the following keeps P09/P10 unavailable:

- quiet watcher or elapsed time treated as currentness proof;
- gap acknowledgement published after current-query admission;
- partial inventory closes a gap;
- stale indexed candidate emitted after live-head mismatch;
- unsaved byte found in any forbidden sink;
- unsaved attach/replacement has a window where older base is unshadowed;
- restart reconstructs unsaved content or keeps its public token valid;
- durable handle targets unsaved bytes;
- parser/provider identity is incomplete or mutable;
- parser executes repository build/macro/LSP/shell/network behavior;
- tolerant syntax fact labeled compiler-verified;
- cfg variants collapsed or unknown predicate treated unconditional;
- malformed/cancelled/budget-limited parse labeled complete;
- missing raw output, mandatory `UNAVAILABLE` probe or self-review.

## Evidence products

Each probe records exact command/fixture, commit/API/config/provider identities, platform/resource
envelope, start/end time, `PASS | FAIL | UNAVAILABLE`, raw output digest and independent reviewer.
Prose-only evidence is rejected.

## Current disposition

```text
source reconcile implementation: ABSENT
watcher/USN provider: UNSELECTED
IDE buffer adapter: UNSELECTED
unsaved sink audit: NOT_EXECUTED
Rust parser/grammar profile: UNSELECTED
structural fixture corpus: DESIGNED_NOT_EXECUTED
probe results: UNAVAILABLE
P09 current workspace: BLOCKED
P10 Rust structure: BLOCKED
```
