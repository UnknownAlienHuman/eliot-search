# Configuration

The [architecture](../architecture/README.md) explains the product; the
[implementation guide](../implementation/README.md) defines the current refactoring target.
Configuration exists to construct those owners, not to certify them.

## Implementation documents

- [Config source/deletion map](../../crates/search-config/FUNCTIONS.md)
- [Typed TOML replacement](../../crates/search-config/TOML_PROFILE_2.md)
- [Runtime-change semantics](RECONFIGURATION_1.1.md)
- [Concrete Qdrant settings](../../config/sections/qdrant_process.md)
- [Storage/credential settings](../../config/sections/secrets.md)

## Target

```text
capture one invocation
→ bounded typed TOML + selected env/CLI overrides
→ concrete DaemonSettings
→ validate final values
→ construct owners/handlers
→ status reads running settings and actual component state
```

Use only fields consumed by the current application. Paths remain OS strings. Unknown fields/types/ranges fail
before side effects. The file is bounded before TOML decoding. Optional components are configured only when
selected and implemented.

Startup is not reconfiguration of a defaults-only runtime. No fingerprint, action plan or receipt is required
to use a non-default value. Search readiness is derived from actual dependencies/handlers, not
`search_accepted`, `indexed_accepted` or optional-gate booleans.

## Source-backed removal

Current `search-config` plus daemon config composition contains about **208 KiB production Rust before tests**:
custom parser/document tree, registry/descriptors, provenance, canonical identity/fingerprint, generic merge/
diff/action planning, receipt-gated activation, redacted full-snapshot status and accepted-readiness records.
Replace it rather than porting it.

Keep concrete settings, useful bounds/input capture and direct status. Delete generic config trees, registry,
fingerprints, receipts and compatibility fixtures after consumers move.

## Historical references

[CONFIGURATION_1.0.md](CONFIGURATION_1.0.md), W5–W10 packets,
[sections.toml](../../config/sections.toml), [section packets](../../config/sections/) and the old example contain
proposals and pre-release lifecycle rules. They are not the supported current schema. Update/delete them with
the typed cutover; old wave/section ownership, fingerprint equality and receipt requirements do not constrain
implementation.
