# P00 / W0 bootstrap

P00 is the only architecture-authorized implementation entry point. W1+ remains blocked.

## Writer A — `search-contracts`

Read all P00 contract-pack files and deliver:

- strong IDs and exact tagged field-level schemas;
- eleven recipe requests and eleven field-level result variants;
- validated-candidate-only result semantics and explicit validation gaps;
- public/protocol/contract reason namespaces;
- canonical JSON/CBOR validation and golden fixtures;
- no I/O, ports or vendor types.

## Integration checkpoint A

1. verify architecture and contract-pack hashes;
2. resolve every challenge;
3. review schema/serialization/result fixtures;
4. publish accepted contracts commit and API/schema digest.

## Writers B and C — after checkpoint A

- `search-domain`: pure transitions, eligibility, fingerprints, ordering and coverage.
- `search-ports`: shared vendor-neutral traits and conformance fakes.

They may run separately from the same immutable contracts handoff.

## Integration checkpoint B

Pin the real Windows-compatible toolchain/dependencies, generate `Cargo.lock`, run formatting,
workspace/W0 tests, dependency/public-API guards and license policy, then publish W0 receipt.

## Required evidence

```text
architecture_contract_challenge_passed
p00_contract_pack_hash_receipt
cargo fmt --check
cargo check --workspace
cargo test -p search-contracts -p search-domain -p search-ports
cargo deny check
recipe_set_exact_test
recipe_result_union_exact_test
emitted_candidate_always_validated_test
validation_gap_contains_no_evidence_test
wire_state_spelling_fixture
forbidden_epoch_sentinel_test
source_owner_generation_digest_fixture
source_occurrence_sequence_u64_fixture
membership_array_schema_rejection_test
unknown_load_bearing_field_fails_closed
canonical_json_cbor_roundtrip_fixtures
public_reason_registry_exact_test
port_operation_semantics_complete
public_vendor_type_guard
dependency_direction_guard
```

Unavailable checks do not become a green receipt.
