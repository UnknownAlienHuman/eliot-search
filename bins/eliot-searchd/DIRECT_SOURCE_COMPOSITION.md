# DIRECT and Git source-composition boundary

Live plaintext DIRECT ingestion and no-execute Git qualification now enter the
same canonical source-owner kernels before any revision bytes reach storage or
any journal event is appended.

```text
safe-reader observation / validated loose Git object
        |
        v
daemon bounded classifier
        |
        v
search-source-admission
  validate -> evaluate -> receipt -> verify
        |
        v
search-source-identity
  stable identity -> source/revision IDs
        |
        v
search-source-registry legacy append plan
        |
        v
daemon durable write/sync/readback
```

Ownership is strict:

- `eliot-searchd` owns filesystem/Git observation adapters, bounded path
  classification, data-root exclusion, storage publication, sync, readback and
  quarantine;
- `search-source-admission` owns canonical policy normalization, observation
  validation, decisions, reason ordering and immutable receipt verification;
- `search-source-identity` owns stable-identity matching, legacy DIRECT
  source/revision formulas and the repository-plus-object Git stable digest;
- `search-source-registry` owns the legacy journal schema, chain validation,
  collision checks and idempotent append planning;
- `search-safe-reader` owns no-execute Git object validation and never invokes
  hooks, filters, credential helpers, shell commands or network fetches.

`source_composition.rs` is now a bounded Git adapter only. It imports the same
canonical composition used by live DIRECT ingestion and contains no private
admission policy, receipt implementation, source-ID formula, revision-ID
formula or registry state machine. Git paths classify admission only; durable
identity is the admitted repository digest plus the exact object ID. Lineage
kind/evidence is retained as metadata and does not silently fork identity.

Manual checks:

```powershell
cargo test --locked -p search-source-admission default_deny
cargo test --locked -p search-source-identity legacy_digest
cargo test --locked -p search-source-identity git_digest
cargo test --locked -p eliot-searchd --test source_composition_owner_boundary
cargo test --locked -p eliot-searchd --test git_source_owner_boundary
cargo test --locked -p eliot-searchd --test git_source_process
cargo check --locked -p eliot-searchd --bin eliot-searchd
cargo test --locked -p eliot-searchd --bin eliot-searchd denied_sources_never_reach_cas
cargo test --locked -p eliot-searchd --bin eliot-searchd rename_preserves_stable_identity
```

These checks establish code ownership and behavioral regression coverage only.
They do not accept W2, issue a gate/receipt, or claim production readiness.
