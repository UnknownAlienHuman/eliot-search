# DIRECT source-composition boundary

The live plaintext DIRECT path enters canonical source-owner kernels before any
revision bytes reach storage or any journal event is appended.

```text
safe-reader observation
        |
        v
daemon path classifier
        |
        v
search-source-admission
  validate -> evaluate -> receipt -> verify
        |
        v
search-source-identity
  resolve stable identity -> derive source/revision IDs
        |
        v
search-source-registry legacy append plan
        |
        v
daemon durable write/sync/readback
```

Ownership is strict:

- `eliot-searchd` owns filesystem observation, bounded path classification,
  data-root exclusion, storage publication, sync, readback and quarantine;
- `search-source-admission` owns canonical policy normalization, observation
  validation, decisions, reason ordering and immutable receipt verification;
- `search-source-identity` owns stable-identity matching and the legacy DIRECT
  source/revision identifier formulas;
- `search-source-registry` owns the legacy journal schema, chain validation,
  collision checks and idempotent append planning.

The old `source_composition.rs` remains temporarily isolated as
`git_source_composition` because the Git object-process qualification imports
its bounded no-execute helpers. It is not the live DIRECT ingestion module.
Removing that remaining compatibility surface is a separate source-acquisition
ownership slice.

Manual checks:

```powershell
cargo test --locked -p search-source-admission default_deny
cargo test --locked -p search-source-identity legacy_digest
cargo test --locked -p eliot-searchd --test source_composition_owner_boundary
cargo check --locked -p eliot-searchd --bin eliot-searchd
cargo test --locked -p eliot-searchd --bin eliot-searchd denied_sources_never_reach_cas
cargo test --locked -p eliot-searchd --bin eliot-searchd rename_preserves_stable_identity
```

These checks establish code ownership and behavioral regression coverage only.
They do not accept W2, issue a gate/receipt, or claim production readiness.
