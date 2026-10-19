# search-source-admission

One pure bounded policy decision before and after a stable source read. It does not own native containment,
source identity, immutable storage, current source state, receipts or historical provenance.

Read the [architecture](../../../docs/architecture/README.md),
[implementation guide](../../../docs/implementation/README.md) and [source map](FUNCTIONS.md).

## Verified current source

`src/` contains about **367 KiB** of Rust:

```text
lib.rs                 165,719 B
policy_v2.rs            45,976 B
observation_v2.rs       34,038 B
evaluation_v2.rs        67,133 B
batch_v2.rs             11,471 B
original_envelope.rs    27,419 B
original_inputs_v2.rs   12,034 B
bootstrap_v1.rs          3,562 B
```

The connected daemon uses `evaluate_pre_read_v2`, but first reconstructs a full original observation from
canonical input arrays. After reading the file it reconstructs another observation, issues/verifies a full
`OriginalAdmissionV2` through provenance/receipt packages and stores that archive in source registry records.
The same `AcquiredNativeSource` does **not** retain the acquired body for immutable storage.

The package also contains v1 compatibility, canonical input/decision identities, complete batch evaluation
and many receipt-oriented fixtures. Search found no normal product caller for `evaluate_batch_v2`.

## Small current policy

The product needs three pure operations:

```text
compile_policy(current settings) -> CompiledAdmissionPolicy

pre_read(policy, CandidateBeforeRead) ->
    AllowRead | Deny | ReviewRequired | Unsupported | Incomplete

finalize(policy, pre_read_allow, CandidateAfterRead) ->
    Allow { classification } | Deny | ReviewRequired | Unsupported | Incomplete
```

`CandidateBeforeRead` contains only bounded facts already observed by the native source owner: root-relative
locator, configured source class, file type, location class and metadata needed by the current policy.
`CandidateAfterRead` adds only facts which require the exact acquired bytes: actual length, binary/text and
encoding/content classification.

The source owner remains responsible for:

- selected-root containment and no-follow final-handle traversal;
- hard-link/location/own-data-root policy tied to native observations;
- stable file identity and read-race detection;
- hashing and retaining the exact body;
- current source-head transaction.

Admission does not reproduce or hash those observations into another authority object.

## Current state

A current `SourceRecord` may retain only useful bounded diagnostics:

```text
policy revision/profile actually applied
closed outcome and reason code
selected final classification
```

It does not retain complete policy rules, original input arrays, canonical observation preimages, decision
fingerprints or a permanent receipt. Equal pure inputs can be evaluated again. A policy change affects future
inventory/reconcile work; removal/update of an existing source remains the source owner's decision.

## Remove after direct caller migration

- v1 policy/observation/decision/receipt/batch compatibility;
- `OriginalAdmissionV2`, original envelope/input arrays and their canonical identities;
- 1,024-occurrence batch identity/result framework;
- `search-source-admission-receipt` and `search-source-provenance` from current ingestion;
- v4 registry/source-record archive/reconstitution codecs;
- compatibility and receipt fixtures which protect those types;
- package-size/Ready/review bureaucracy embedded in local instructions.

Keep one reviewed pattern matcher and one small evaluator. If the useful result fits naturally beside source
inventory, move it there and delete this package rather than retaining a framework boundary.

## Completion

A denied candidate performs zero body reads. An allowed candidate is read once through the stable native
reader; the same owned bytes pass to immutable retention and post-read classification; one typed current
source head is committed; no provenance/receipt/original-input object is constructed.

Run minimal Rust 1.98 compiler/Clippy for the changed admission/source/daemon consumers after code. Focused
cases cover deny-before-read, mandatory exclusions, incomplete facts, post-read narrowing, cancellation and
content-free diagnostics. Historical byte/golden/batch matrices are removed with their APIs.
