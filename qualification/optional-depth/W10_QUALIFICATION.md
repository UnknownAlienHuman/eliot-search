# W10 optional-depth qualification contract

**Status:** `DISABLED_NOT_SELECTED`  
**Stage / gate:** P16-P18 / G6  
**Architecture:** ELIOT Search 8.4 S17.2, S21.2, S29, S35, S37 and P16-P18  
**Profiles:** model, document and scale are accepted independently.

## 1. Candidate-specific acceptance

G6 is not one global switch. One evidence run selects exactly one candidate:

```text
model:<exact profile digest>
document:<exact profile digest>
scale:<exact topology profile digest>
```

The five central G6 evidence IDs are required for that selected candidate. Evidence for another provider,
profile, runtime, tokenizer, quantization, document engine or topology cannot be reused unless every
load-bearing identity and fixture digest is equal and the reviewer explicitly accepts equivalence.

## 2. Prerequisite chain

Before any optional provider code/artifact is admitted to a qualification environment:

1. exact accepted P15 Product Pulse report and independent reviewer receipt;
2. dedicated candidate ADR fixing the intended use, artifact/dependency/runtime, content/security model,
   feature/configuration names, expected benefit, removal path and migration/rollback scope;
3. accepted license/source/supply-chain review;
4. exact Windows package and artifact identities;
5. pre-registered extension of the P15 evaluation policy and candidate fixtures;
6. baseline regression fixture and removal plan;
7. candidate-specific integration ticket with one writer per package and immutable direct handoffs.

If any prerequisite is absent, static files remain `DISABLED`; package/worker/daemon implementation is not
authorized.

## 3. Common qualification sequence

1. Verify P15/ADR/artifact/profile/fixture/policy digests.
2. Build the candidate only under the named non-default Cargo feature.
3. Prove binary/profile absence and accepted P15 behavior with the feature disabled.
4. Start candidate workers only inside a private Windows qualification environment.
5. Execute exact artifact/runtime/profile golden, security/content and resource probes.
6. Stage candidate capability without changing the serving route/configuration.
7. Execute candidate-specific migration or reviewed not-applicable proof.
8. Execute the pre-registered incremental Product Pulse comparison.
9. Execute worker/provider/fault/cancellation/access/currentness/leakage tests.
10. Execute complete removal/rollback and accepted P15 regression.
11. Publish immutable raw evidence and independent reviewer receipt.
12. Only then may the integration owner issue a candidate-specific G6 acceptance receipt.

A green package test, worker readiness, successful model inference/materialization or collection build is
not activation or acceptance.

## 4. Model candidate

The exact model profile freezes provider/runtime/model/tokenizer/templates/truncation/pooling/vector
layout/dimensions/dtype/normalization/distance/quantization/rerank calibration/resource/cache/content
policy and golden fixture.

Mandatory properties:

- no network, auto-download/update, provider training/learning or persistent content cache;
- source/query/unsaved content is bounded, process-memory-only and content-minimized in diagnostics;
- document/query encoding pair and rerank semantics match the exact golden fixture;
- all vectors/scores are finite, bounded and independently validated;
- dense/multivector candidate identity creates a new projection/collection generation;
- rerank is a subset-only transform and cannot add authority/evidence/completeness;
- inaccessible/staged/retired/denied/purged/shadowed material cannot influence output;
- provider failure leaves accepted P15 serving available with explicit optional gap;
- measured material benefit and cost/risk are accepted independently;
- uninstall stops the worker, clears allowed cache/temp state and restores P15 regression.

## 5. Document candidate

The exact document profile freezes provider/runtime/Windows packaging/input set/container policy,
page/object/image/table/figure/decompression limits, no-execute/no-network rules, output schema,
coordinates/loss maps, assurance and resource/temp policy.

Mandatory properties:

- no scripts, JavaScript, VBA/macros, OLE actions, hooks, filters, shell/child process, remote resource or
  credential prompt;
- no absolute/traversal/device/alternate-stream/symlink/hardlink/reparse escape;
- archive/page/object/image/decompression/output bombs are bounded;
- malformed/fuzz input cannot crash or corrupt the daemon;
- exact retained revision digest/length; no current-path or Qdrant-payload substitution;
- coordinate/loss maps and assurance are independently validated;
- materialization/profile change uses a new representation/projection/collection generation;
- measured quality/fidelity gain and cost/risk are accepted independently;
- provider removal restores accepted P15 text/code behavior and clears temp/cache state.

## 6. Scale candidate

The exact scale profile freezes Qdrant server/client artifacts, selected node/process/shard/replication/
write-consistency topology, strict schema, scoring/IDF/query-fanout semantics, resource/failure model,
migration barriers, route pins, reclaim and rollback.

Mandatory properties:

- accepted report proves one-shard is the material bottleneck after ordinary tuning;
- active collection topology/schema is never changed in place;
- candidate generation follows base-at-R0, ordered catch-up, final barrier at R1, exact validation and
  guarded redb route switch;
- access/currentness/shadow/purge/filter/IDF and scoring semantics are equivalent or a new product/scoring
  profile receives its own acceptance evidence;
- every migration-state and acknowledgement boundary is kill/reopen tested;
- failed candidates are exact-discarded without changing visible route;
- old route is retained until route/epoch pins drain and rollback policy releases it;
- post-switch rollback is a forward guarded route transition, never epoch rewind;
- measured throughput/latency gain exceeds accepted resource/quality cost.

## 7. G6 evidence interpretation

Each selected candidate must produce all five central evidence records:

1. `dedicated_optional_profile_adr`
2. `exact_provider_artifact_qualification`
3. `measured_material_benefit`
4. `removal_or_uninstall_fallback`
5. `migration_and_rollback_when_applicable`

For `RERANK_ONLY`, evidence 5 is still present and may PASS only as an independently reviewed
`NOT_APPLICABLE_NO_PERSISTENT_SCHEMA` receipt proving no persistent vector/representation/route state,
while worker/config rollback remains covered by evidence 4. `UNAVAILABLE`, missing or prose-only evidence
cannot pass.

## 8. Evidence record

Every candidate probe result contains candidate/profile/P15/ADR/repository/API/configuration/Windows/
artifact/fixture identities, command digest, `PASS | FAIL | UNAVAILABLE`, immutable raw-output ref/digest,
timestamps and independent reviewer receipt. Source/query/unsaved/secret/token/path content is absent.

Static `DISABLED` templates are not evidence. The integration owner creates append-only candidate-run
records; package writers cannot edit accepted evidence or self-review.

## 9. Activation evidence

After G6 acceptance, activation still requires:

- compiled named feature and explicit config;
- current binding authorization;
- exact qualified worker/process identity;
- candidate route/profile/config validation;
- guarded control commit and coherent capability snapshot;
- bounded restart/quarantine policy;
- tested deactivation/removal command.

An accepted provider artifact is not automatically active.

## 10. Stop conditions

Stop without G6 acceptance on:

- absent/mismatched P15 or ADR;
- any unselected/floating artifact/runtime/profile/topology or license gap;
- network/download/update/training/content-retention requirement;
- provider output treated as evidence/exact proof/client authority;
- rerank widening candidates or scope;
- document execution/remote resource or malformed-input isolation failure;
- access/currentness/content leakage or noninterference failure;
- material benefit or resource report incomplete/insufficient;
- baseline removal/rollback regression;
- scale without measured bottleneck or any incomplete migration/pin/rollback evidence;
- missing raw output or independent review;
- package/worker/daemon self-acceptance.

## 11. Current disposition

```text
accepted P15 receipt: UNSELECTED
candidate ADR: UNSELECTED
model profile/artifact/runtime: DISABLED / UNSELECTED
model probes: 15 DISABLED
document profile/artifact/runtime: DISABLED / UNSELECTED
document probes: 15 DISABLED
scale profile/topology: DISABLED / UNSELECTED
scale probes: 15 DISABLED
G6 candidate: NONE
optional depth: NOT AUTHORIZED
```
