# Context artifact candidate v1 qualification

This suite qualifies only the deterministic non-authoritative candidate builder.

## Commands

```powershell
python -m py_compile `
  tools/build-context-artifact-candidate.py `
  tools/context_artifact_builder_v1/core.py `
  tools/context_artifact_builder_v1/bundle.py `
  tools/context_artifact_builder_v1/extract.py `
  tools/context_artifact_builder_v1/build.py `
  qualification/context-artifact/test_context_artifact_candidate_v1.py

python qualification/context-artifact/test_context_artifact_candidate_v1.py
cargo test --locked -p xtask --test context_artifact_validation
cargo run --locked --quiet -p xtask -- validate context-artifact-candidate --json
```

The Rust structural validator checks component registry, schema, digest profile,
twenty-case inventory, manual workflow policy, artifact-root fencing and the
all-false authority boundary. It does not invoke the Python builder or write a
candidate. The separate twenty-case corpus and workflow build continue to
exercise immutable Git-tree extraction, bundle construction and idempotent
artifact writes until the full builder is ported.

The workflow still builds `search-contracts` against an exact commit. Expected output:

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

`cases-v1.toml` inventories twenty cases covering immutable-tree determinism, line-ending normalization,
length framing, source and selector failures, accepted handoff requirements, current-package conflicts,
output fencing/idempotency, digest separation, unresolved manifest fields and the authority ceiling.

## Evidence ceiling

A green result is not:

- an immutable artifact-store write or `ImmutableArtifactRef`;
- a committed `context_manifest_v1`;
- a materializer or reviewer signature;
- an assignment ticket, writer lease or acknowledgement;
- package/G0/W0 acceptance;
- implementation authority.
