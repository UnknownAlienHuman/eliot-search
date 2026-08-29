# W10 optional depth contracts 1.0

**Status:** implementation projection only; P16-P18/G6 remains blocked.  
**Architecture:** ELIOT Search 8.4, S17.2, S21.2, S29, S35, S37, H17/P16-P18.  
**Rule:** optional depth may add bounded nomination, ranking or materialization capability; it never
changes source truth, access, currentness, exact-proof or client-authority semantics.

## 1. Candidate classes

W10 has three independently selectable candidate classes:

```text
MODEL_OPTIONAL
  ├─ RERANK_ONLY
  ├─ DENSE_VECTOR
  └─ MULTIVECTOR

DOCUMENT_OPTIONAL
  └─ one exact isolated document materializer profile

ADVANCED_SCALE_OPTIONAL
  └─ one exact measured Qdrant topology/profile migration
```

A G6 acceptance is candidate-specific. Accepting one model profile does not accept a document provider,
another model, another tokenizer/runtime/quantization, or a scale topology. Each distinct load-bearing
identity requires its own ADR, qualification and evidence receipt.

## 2. Gate chain

A candidate may enter qualification only when all of these immutable inputs exist:

1. accepted P15 Product Pulse report and independent reviewer receipt;
2. dedicated candidate ADR fixing scope, risks, dependency/artifact selection and removal path;
3. exact Windows-compatible provider/artifact/dependency/license identity;
4. compiled Cargo feature named by the ADR;
5. explicit disabled-by-default configuration profile;
6. binding/profile authorization for any exposed capability;
7. accepted measurement and evidence plan derived from the P15 corpus;
8. migration and rollback plan, or an independently reviewed not-applicable proof for a stateless
   rerank-only candidate.

Configuration, package presence, a green unit suite, model popularity or version text cannot satisfy the
gate.

## 3. Activation state machine

```text
DISABLED
  -> ELIGIBLE_FOR_QUALIFICATION
  -> QUALIFICATION_RUNNING
  -> QUALIFIED_NOT_MEASURED
  -> BENEFIT_EVALUATION_RUNNING
  -> CANDIDATE_ACCEPTED
  -> STAGED_NOT_SERVING
  -> MIGRATING               # when persistent representation/schema changes
  -> VALIDATED_NOT_ACTIVE
  -> ACTIVE

any non-active state -> FAILED | DISABLED
ACTIVE -> DRAINING -> REMOVED
any state -> QUARANTINED
```

Only the daemon composition owner changes serving capability. Workers and provider packages cannot
self-activate. `ACTIVE` is published only after the exact gate/profile, worker, route/profile and
configuration receipts are coherent in one control snapshot.

A failed stage leaves the accepted P15 baseline authoritative. No candidate is an implicit fallback for
another candidate.

## 4. Model profile identity

`ModelProfileDescriptor` is immutable and includes at least:

```yaml
profile_id_and_revision: object
candidate_kind: rerank_only | dense_vector | multivector
provider_artifact_identity: object
model_weights_identity: object
runtime_backend_and_version: object
Windows_packaging_identity: object
tokenizer_and_preprocessor_identity: object
query_template_and_document_template: object
input_modalities_and_encoding: object
input_truncation_and_chunk_policy: object
pooling_or_token_vector_layout: object
output_dimensions_and_named_vectors: object
output_dtype_normalization_and_distance: object
quantization_and_calibration_identity: object
rerank_score_semantics_and_tie_break: object
batch_concurrency_deadline_and_memory_bounds: object
cache_and_content_retention_policy: object
license_and_source_receipts: object
golden_fixture_digest: digest
profile_digest: digest
```

Any change to a listed behavior creates a new profile identity. `latest`, runtime defaults and model-name
strings without exact artifacts are rejected.

Document and query encoders are qualified as one pair. Rerank calibration is qualified against the
exact candidate ordering/input template. Dense/multivector schema identity is part of
`ProjectionProfileSetId` and the collection generation.

## 5. Model semantics

### 5.1 Candidate nomination only

Dense and multivector retrieval nominate bounded candidates. They do not prove exact identity,
completeness, absence, source content or correctness. Every candidate still passes the same access,
currentness, overlay shadow, exact-revision readback and emission validation as lexical candidates.

### 5.2 Rerank is a closed transform

Rerank consumes only a finite already-authorized candidate set and may reorder or drop within that set.
It cannot:

- add a source, membership or candidate;
- widen requested/authorized scope;
- inspect inaccessible candidates;
- change a source handle or evidence role;
- turn a validation gap into evidence;
- label candidate scope complete;
- emit a generative answer or client disposition.

Timeout/cancellation/provider failure preserves the pre-rerank candidate order or returns an explicit
optional-leg gap according to the accepted plan. It never silently invokes another model.

### 5.3 Access and scoring noninterference

Model input construction occurs after server-authoritative access/currentness/shadow planning. Dense
retrieval filters use the same base eligibility obligations as lexical retrieval. An inaccessible,
staged, retired, denied, purged or shadowed point cannot affect candidate nomination, score calibration,
fusion, rerank, diversity, counts or traces.

A restrictive change contaminating a model leg discards that whole influenced leg; post-filter cleanup
is forbidden.

### 5.4 Content handling

Source/query/unsaved content is supplied only in a bounded process-memory request under current
authorization. It cannot enter:

```text
provider training/fine-tuning/learning
persistent model cache
telemetry or crash attachments
worker command line or environment
backup or restore artifacts
evaluation/training corpus outside the accepted P15 fixture flow
```

Unsaved content is never persisted. Any provider requiring network upload or vendor-side retention is
outside this local baseline and needs a different architecture/gate.

## 6. Model worker boundary

`eliot-search-model-worker` is absent or stopped by default. The daemon launches the exact qualified
binary under a private authenticated worker channel and Windows lifecycle/resource containment.

The worker receives only typed bounded encode/rerank requests and opaque request/profile identities. It
has no redb, CAS, Qdrant, source inventory, handle store, secret-store or client connection.

Required worker controls include:

- exact binary/runtime/model/profile verification before readiness;
- one accepted profile per process incarnation unless the ADR explicitly qualifies a bounded set;
- finite input, batch, queue, concurrency, memory/commit, CPU/GPU and deadline ceilings;
- cooperative cancellation plus bounded process termination;
- content-minimized stdout/stderr/health;
- no network, automatic artifact download or background update;
- private temporary/cache directory with explicit zero persistent-input policy;
- crash isolation and deterministic quarantine/restart ceiling;
- removal receipt proving process exit and provider-input/cache cleanup.

Worker readiness is not capability activation; daemon gate and route state remain authoritative.

## 7. Persistent model vectors and collection generations

`RERANK_ONLY` has no persistent model vectors and may carry an accepted not-applicable migration receipt.
`DENSE_VECTOR` and `MULTIVECTOR` require a new collection generation because named-vector schema,
projection profile set and point identity change.

The migration follows the existing generation protocol:

```text
candidate collection/profile admitted
-> base build at R0
-> ordered change-log catch-up
-> final barrier at R1
-> exact schema/point/readback/noninterference validation
-> guarded redb route/profile/config commit
-> capability snapshot publication
-> old route drain under pins
-> exact old-route reclaim
```

No in-place active schema mutation, alias-as-commit or point reinterpretation is allowed.

## 8. Document provider identity

`DocumentProviderProfile` is immutable and includes:

```yaml
profile_id_and_revision: object
provider_binary_library_runtime_identity: object
Windows_packaging_and_license: object
accepted_input_mime_and_encoding_set: object
container_archive_member_policy: object
page_object_image_table_figure_limits: object
nested_depth_and_decompression_ratio_limits: object
script_macro_hook_external_resource_policy: object
network_and_credential_policy: object
text_structure_table_figure_output_schema: object
native_anchor_coordinate_spaces: object
coordinate_and_loss_map_identity: object
assurance_ceiling_and_warning_taxonomy: object
temporary_file_and_memory_policy: object
cancellation_deadline_and_resource_bounds: object
golden_fuzz_fixture_digest: digest
profile_digest: digest
```

Provider name/version alone is insufficient. Any parser/rendering, coordinate, loss, OCR, layout,
archive, limit or assurance change creates a different profile.

## 9. Document worker no-execute boundary

`eliot-search-doc-worker` is absent or stopped by default and processes one exact retained source
revision per bounded request. It cannot execute or fetch:

```text
scripts, JavaScript, VBA/macros, OLE actions
build hooks, filters or shell commands
embedded executables or external applications
remote fonts, images, schemas, styles or links
network URLs or credential prompts
archive members as programs
```

It rejects path traversal, device paths, symlink/hardlink/reparse escape, nested archive bombs, excessive
pages/objects/images/dimensions, decompression bombs and malformed encodings according to the qualified
profile.

Output is accepted only after exact source digest/length binding, bounded materialized bytes, output
schema validation, coordinate/loss-map verification and assurance classification. Current-path or
Qdrant payload substitution is forbidden.

Temporary files are private, bounded and removed after verification or failure. Provider crashes,
timeouts and malformed input cannot crash or corrupt the daemon. A failure returns a typed materializer
gap; lexical/code baseline behavior remains available where applicable.

## 10. Document projection migration

A document profile changes representation/materialization identity and therefore requires explicit
re-preparation and a new projection/collection generation for indexed document content. The old baseline
route remains authoritative until exact candidate validation and guarded route/config activation.

Removal switches serving back to the accepted baseline profile, drains route pins, stops the worker,
invalidates profile-specific caches/handles where required and reclaims only exact optional artifacts
after protection watermarks permit it.

## 11. Measured material benefit

Each optional candidate is compared to the exact accepted P15 baseline on a pre-registered extension of
the Product Pulse corpus. `search-eval` retains metric/verdict meaning; the candidate package only emits
bounded instrumentation.

Acceptance requires an independent report demonstrating material incremental benefit for the candidate's
declared use cases at an accepted cost/risk envelope. It reports at least:

- quality/recall/false-positive or materialization-fidelity gain;
- latency to first useful result and steady-state tail;
- CPU/GPU, RAM/VRAM, disk and preparation cost;
- source reads and model token/input/output volumes where applicable;
- access/currentness/content leakage audits;
- fault, cancellation and worker-crash behavior;
- fallback/removal regression against P15.

No package chooses its own threshold after seeing results. A candidate that is merely different, more
expensive without material gain, or unsafe remains disabled.

## 12. Removal and baseline restoration

Every candidate ships with a tested removal path before activation.

```text
block new optional work
-> publish capability unavailable/draining state
-> finish or cancel bounded in-flight requests
-> route new requests to accepted P15 baseline
-> commit baseline profile/config/route snapshot
-> drain old optional route pins
-> stop worker and verify process exit
-> clear optional content caches/temp files
-> exact reclaim optional projections when safe
-> run P15 regression fixture
-> issue RemovalReceipt
```

The receipt binds candidate/profile/artifact, previous and restored route/config fingerprints, worker
shutdown, cache cleanup, exact reclaimed/deferred manifests and P15 regression digest. It never claims
secure erase without evidence.

Provider unavailability before removal narrows optional coverage; it does not make baseline packages
depend on the provider or prevent DIRECT/LEXICAL/CODE serving.

## 13. Advanced scale trigger

P18 cannot begin because an operator expects growth. It requires an accepted, reproducible report proving
that the qualified one-node/one-shard profile is the material bottleneck after ordinary tuning and that
the proposed scale change is preferable to simpler resource/configuration changes.

`ScaleProfileDescriptor` fixes:

```yaml
exact_qdrant_server_client_artifacts: object
node_process_topology: object
shard_replication_write_consistency: object
vector_payload_and_strict_schema_identity: object
scoring_idf_and_query_fanout_semantics: object
resource_and_failure_model: object
migration_catch_up_and_barrier_policy: object
route_pin_drain_and_reclaim_policy: object
rollback_and_candidate_discard_policy: object
equivalence_fixture_digest: digest
profile_digest: digest
```

No topology is selected in this scaffold.

## 14. P18 migration state machine

```text
SCALE_PLANNED
-> CANDIDATE_CREATED
-> BASE_BUILT_AT_R0
-> CHANGELOG_CATCHING_UP
-> FINAL_BARRIER_ENTERED
-> VALIDATED_AT_R1
-> ROUTE_SWITCH_COMMITTED
-> OLD_ROUTE_DRAINING
-> OLD_ROUTE_RECLAIMABLE
-> COMPLETE
```

Recovery states:

```text
CANDIDATE_FAILED
CANDIDATE_DISCARDED
ROLLBACK_PENDING
ROLLED_BACK
SCALE_BLOCKED
QUARANTINED
```

The guarded redb route switch is the only serving linearization point. Qdrant alias movement is not a
Search commit. Route/epoch pins protect old and new routes. A failed candidate never changes visible
route and is discarded through exact manifests.

Rollback before route switch discards the candidate. Rollback after route switch is another guarded
route transition to a fully validated retained/rebuilt baseline route; it never rewinds epochs or
reinterprets points in place.

## 15. P18 equivalence and fault evidence

Advanced scale must prove, for the selected topology:

- eligibility, access/currentness/shadow/purge and filtered-IDF noninterference equivalence;
- deterministic plan/fusion/tie-break semantics or an explicitly new accepted scoring profile;
- exact counts/readback and publication semantics;
- no current point before committed route/epoch state;
- every migration-state kill/reopen recovery;
- ordered catch-up and final barrier with no lost/duplicated source change;
- old-route pin drainage before reclaim;
- failed-candidate discard and post-switch rollback;
- bounded query fanout, queues, memory, disk and background load;
- measured latency/throughput benefit and no unacceptable quality regression.

If cross-shard scoring/IDF behavior cannot satisfy the accepted profile, the scale candidate is rejected
or treated as a new scoring/product profile requiring its own Product Pulse evidence.

## 16. Capability and configuration publication

Optional configuration is staged, never applied directly. The daemon composes these obligations:

```text
GATE_REQUIRED
DRAIN_AND_RESTART or worker start
NEW_COLLECTION_GENERATION / REBUILD_PROJECTION when applicable
SECURITY_BARRIER for binding/capability changes
ROUTE_DRAIN_AND_RECLAIM on removal or topology replacement
```

A candidate config fingerprint becomes authoritative only after every required receipt succeeds. Failed
or partial activation leaves the previous baseline snapshot authoritative.

The binding-filtered capability descriptor reports only coherent states:

```text
DISABLED | QUALIFYING | AVAILABLE | DEGRADED | DRAINING | QUARANTINED
```

It never reports an optional recipe/vector/materializer as available before the handler, worker,
profile, route and gate receipts are all accepted.

## 17. Package decomposition

- `search-model-provider`: provider-neutral model profile, encode/rerank and output validation.
- `eliot-search-model-worker`: isolated model process and resource/cancellation boundary.
- `eliot-search-doc-worker`: isolated document materializer and no-execute boundary.
- `eliot-searchd`: gate, worker, migration, capability and removal composition.
- `search-qdrant-bridge`: P18 topology/schema/data-plane qualification.
- `search-publication`: P18 migration and guarded route transition.
- `search-epoch-pins`: route pins and drain watermarks.
- `search-index-reclaimer`: exact old-route reclamation after drain.
- `search-eval`: incremental benefit and regression evidence meaning.

No new forwarding crate is created. Each writer edits only its package path; shared qualification and
activation receipts remain integration-owned.

## 18. Hard stop conditions

- accepted P15 receipt absent, stale or mismatched;
- candidate ADR, exact artifact/profile/license or Windows qualification absent;
- candidate feature/config disabled or binding unauthorized;
- any network, automatic download/update, training/learning or content-retention path;
- unsaved content persistence or content-bearing telemetry/crash output;
- provider result used as source evidence, exact identity or complete-negative proof;
- model rerank widens candidate set or scope;
- document provider executes code or follows remote resources;
- persistent profile/schema changed in place;
- material benefit, resource cost or safety report incomplete;
- removal does not restore accepted P15 behavior;
- scale begins without a measured one-shard bottleneck;
- migration/route-switch/pin-drain/rollback evidence incomplete;
- package self-accepts G6 or configuration claims activation.

## 19. Current disposition

```text
accepted P15 receipt: UNSELECTED
model profile/artifact/runtime: DISABLED / UNSELECTED
model worker: ABSENT
model benefit evidence: NOT EXECUTED
document profile/artifact/runtime: DISABLED / UNSELECTED
document worker: ABSENT
document benefit evidence: NOT EXECUTED
scale topology: DISABLED / UNSELECTED
scale bottleneck and migration evidence: NOT EXECUTED
G6: NOT ACCEPTED
baseline DIRECT/LEXICAL/CODE: remains authoritative
```
