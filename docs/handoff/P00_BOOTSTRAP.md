# P00 / W0 bootstrap

P00 is the only architecture-authorized implementation entry point. Do not launch W1 or later packages.

## Writer A — `search-contracts`

Read root/package instructions, its assignment and all files listed for it in
`docs/contracts/p00/README.md`.

Deliver:

- exact strong IDs and tagged field-level schemas;
- eleven-recipe registry and typed request/output families;
- public/protocol/contract reason namespaces;
- canonical JSON/CBOR validation and golden fixtures;
- no I/O, port traits or vendor types.

## Integration checkpoint A

Before another W0 writer starts:

1. verify architecture hash and P00 contract-pack hashes;
2. resolve every contract challenge;
3. review schema/serialization fixtures;
4. publish accepted contracts commit and public API/schema digest.

## Writers B and C — after checkpoint A

They may run in parallel in separate worktrees.

### Writer B — `search-domain`

Consume only the accepted `search-contracts` handoff. Implement pure state transitions, eligibility,
fingerprint, ordering and coverage rules. No I/O or ports.

### Writer C — `search-ports`

Consume only the accepted `search-contracts` handoff plus `PORT_OPERATIONS.md`. Implement shared
vendor-neutral traits and conformance fake interfaces. No adapter, runtime state, executor choice or
vendor dependency.

## Integration checkpoint B

- verify accepted API digests for contracts/domain/ports;
- pin a Windows-compatible stable Rust/dependency set;
- generate `Cargo.lock` and dependency/license evidence;
- run formatting, workspace check, W0 tests, dependency/public-API guards and `cargo deny`;
- publish W0 receipt with exact commands/environment/artifact identities;
- only then advance launch state.

## Required evidence

```text
architecture contract challenge passed
P00 contract-pack hash receipt
cargo fmt --check
cargo check --workspace
cargo test -p search-contracts -p search-domain -p search-ports
cargo deny check
recipe_set_exact_test
forbidden_epoch_sentinel_test
source_owner_generation_digest_fixture
membership_array_schema_rejection_test
unknown_load_bearing_field_fails_closed
canonical_json_cbor_roundtrip_fixtures
public_reason_registry_exact_test
port_operation_semantics_complete
public_vendor_type_guard
dependency_direction_guard
```

Unavailable checks are not converted into a green receipt.
