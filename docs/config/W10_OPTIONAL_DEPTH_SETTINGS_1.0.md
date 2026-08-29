# W10 optional-depth settings 1.0

Machine schema: [`../../config/w10-optional-depth.toml`](../../config/w10-optional-depth.toml).

These settings express a candidate request and finite resource ceilings. They do not select artifacts,
prove P15 acceptance, satisfy G6, activate a worker, switch a route or authorize a binding.

## Field modes

- `LOCKED` — security, authority, migration, removal or baseline invariant; overrides are rejected.
- `TUNABLE` — finite request/resource value applied only to the next candidate stage or worker start.
- `QUALIFIED_REF` — immutable accepted P15/ADR/profile/evidence receipt; `UNSELECTED` blocks staging.

All optional candidates default disabled. Exactly one candidate class may be staged per integration
ticket.

## Gate settings

Candidate activation needs all six qualified references:

```text
accepted P15 receipt
candidate ADR
artifact/profile qualification
measured benefit
removal/fallback proof
migration/rollback proof or reviewed rerank-only not-applicable receipt
```

It also needs the named compiled feature, explicit configuration and current binding authorization.
`configuration_alone_authorizes=false` is locked.

## Model settings

Only bounded batch, queue, concurrency, deadline and cancellation-grace ceilings are tunable. They apply
to the next worker start and are further narrowed by the qualified profile and request budget.

Locked false:

```text
network
automatic download/upgrade
training or learning
generative answer authority
persistent input cache
unsaved persistence
implicit provider fallback
```

Rerank output must remain a subset of its input candidate set. Dimensions, tokenizer, runtime,
quantization and vector layout are profile identity, not free-form settings.

## Document settings

Finite input/output/page/archive/nesting/decompression/temp/concurrency/deadline ceilings are tunable for
the next worker start. The qualified provider may impose stricter values.

Locked false:

```text
network
scripts/macros
shell or child process
remote resources
path escape
automatic download/upgrade
```

MIME support, coordinates, loss maps and assurance are profile identity.

## Scale settings

A scale request requires a qualified topology profile and measured bottleneck receipt. Topology values
are not user-tunable in this file. In-place active schema/topology change and alias-as-commit are locked
false; guarded redb route switch, failed-candidate discard, old-route pin drainage and post-switch
rollback are locked true.

## Removal settings

Baseline route/config restoration and capability draining occur before physical optional reclaim. Worker
exit, input/temp/cache cleanup, route-pin drain and accepted P15 regression are mandatory. Secure erase
is never claimed without evidence.

## Change semantics

`requested` or `candidate_class` changes emit `GATE_REQUIRED`; resource tuning emits
`APPLY_NEXT_WORKER_START`. Profile/reference changes create a new candidate qualification and, for
persistent vectors/document/scale, a new collection generation. No optional activation is `APPLY_LIVE`.

The previous baseline config remains authoritative until the daemon commits and publishes a coherent
gate/handler/worker/profile/route/config snapshot. Failed staging cannot leave a mixed snapshot.

## Required settings tests

- all candidates disabled and qualified refs `UNSELECTED` by default;
- one candidate per ticket and config alone never authorizes;
- locked fields reject file/environment/CLI override;
- every tunable has finite min/max and next-worker-start semantics;
- profile identity cannot be altered through resource settings;
- model network/training/cache/generative/fallback floors cannot change;
- document execute/network/remote/path floors cannot change;
- scale cannot mutate active topology or reclaim a pinned route;
- removal cannot skip baseline restore, worker exit, pin drain, cleanup or P15 regression;
- canonical settings digest is deterministic and redacted of artifacts/paths/content/secrets.
