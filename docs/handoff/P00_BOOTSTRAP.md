# P00 / W0 bootstrap

P00 is the only architecture-authorized implementation entry point. Do not launch W1 or later crates
from this document.

## Roles

### Writer A — `search-contracts`

Read:

- root and package `AGENTS.md`;
- `swarm/ASSIGNMENT_PROTOCOL.md`;
- `swarm/assignments/search-contracts.md`;
- the immutable P00 assignment issue.

Deliver the complete vendor-neutral v1 schema, exact recipe/reason registries, validation and
serialization fixtures. Do not implement I/O or vendor adapters.

### Integration owner checkpoint A

Before Writer B starts:

1. review the public schema and invariant tests;
2. publish an accepted handoff and public API digest;
3. resolve every contract-change request;
4. freeze that dependency commit for downstream use.

### Writer B — `search-domain`

Read:

- root and package `AGENTS.md`;
- `swarm/ASSIGNMENT_PROTOCOL.md`;
- `swarm/assignments/search-domain.md`;
- only the accepted `search-contracts` public API/handoff.

Deliver pure transition, eligibility, fingerprint, ordering and coverage rules. No I/O or vendor
dependency.

### Integration owner checkpoint B

1. verify the embedded Architecture SHA-256:
   `ae4c18ccff256ce4d5fdf91dfd9041236ff6f332b611bae3bd748c2da8ac6a1c`;
2. select/pin a stable Rust toolchain after Windows/dependency compatibility checks;
3. review exact dependency source/license policy and generate `Cargo.lock`;
4. run format, workspace, P00 package tests, dependency direction and `cargo deny`;
5. publish a W0 receipt containing exact commands, environment and commit/API digests;
6. only then advance `swarm/launch-state.toml`.

## Minimum P00 contract inventory

- all strong IDs/newtypes from H3.1;
- `SourceNamespaceOwnership`, `SourceOwnerCutoverReceipt`;
- source identity/revision/membership/materialization/representation/projection records;
- complete residency key and native anchors;
- `SourceView`, `WorkspaceViewRevision`;
- grant, budget, plan, fingerprint and candidate-set records;
- exact-scan plan/report and handle classes;
- provider envelope/capability descriptor;
- security barrier/live-deny identities;
- exactly eleven v1 recipes;
- closed typed reason-code registry.

## Required evidence

```text
architecture contract challenge passed
cargo fmt --check
cargo check --workspace
cargo test -p search-contracts -p search-domain
cargo deny check
recipe_set_exact_test
forbidden_epoch_sentinel_test
membership_array_schema_rejection_test
unknown_load_bearing_field_fails_closed
dependency_direction_guard
public_vendor_type_guard
```

Unavailable commands are not converted into a green receipt. P00 remains incomplete until its real
toolchain/dependency environment exists.
