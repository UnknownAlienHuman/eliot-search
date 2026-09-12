# Context artifact candidate v1 qualification

This suite qualifies only the deterministic non-authoritative candidate builder.

## Commands

```powershell
cargo test --locked -p xtask --test context_artifact_parity
cargo test --locked -p xtask --test context_artifact_module_ownership
cargo test --locked -p xtask --test context_artifact_rust_io
cargo test --locked -p xtask --test context_artifact_builder
cargo test --locked -p xtask --test context_artifact_validation
cargo run --locked --quiet -p xtask -- validate context-artifact-candidate --json

$format = git rev-parse --show-object-format
$commit = git rev-parse HEAD
cargo run --locked --quiet -p xtask -- build context-artifact-candidate `
  --package search-contracts `
  --base-commit "${format}:${commit}" `
  --output-root artifacts/context-artifact-candidates/qualification `
  --print-result
```

Rust owns the complete candidate path:

- immutable Git-tree preflight and exact bounded blob readback;
- schema-v2 ticket/context draft checks and zero-authority fences;
- accepted-handoff verification and supersession checks;
- UTF-8/LF source materialization;
- canonical registry-fragment extraction, including the W0 module packet;
- length-framed bundle rendering plus strict inverse parsing;
- domain-separated bundle/candidate digests;
- candidate assembly;
- output-root fencing and idempotent temp/write/sync/rename/readback publication.

No Python runtime is required for this builder. Captured CPython parity vectors remain ordinary fixtures used by Rust tests; they are not executed.

The workflow builds `search-contracts` against one exact algorithm-tagged commit. Expected output:

```text
status = ARTIFACT_CANDIDATE_NOT_STORED_NOT_SIGNED
reason_codes = []
control_record_mutations = []
artifact format = ELIOT_SWARM_CONTEXT_1
bundle round-trip = true
all authority flags = false
context_manifest_v1 projection = not a schema instance
```

## Corpus

`cases-v1.toml` inventories twenty cases covering immutable-tree determinism,
line-ending normalization, length framing, source and selector failures,
accepted handoff requirements, current-package conflicts, output
fencing/idempotency, digest separation, unresolved manifest fields and the
authority ceiling.

## Evidence ceiling

A green result is not:

- an immutable artifact-store write or `ImmutableArtifactRef`;
- a committed `context_manifest_v1`;
- a materializer or reviewer signature;
- an assignment ticket, writer lease or acknowledgement;
- package/G0/W0 acceptance;
- implementation authority.
