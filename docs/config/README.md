# Configuration

Configuration constructs the current application; it does not authorize it.

## Governing implementation documents

- [Source/deletion map](../../crates/search-config/FUNCTIONS.md)
- [Typed TOML replacement](../../crates/search-config/FUNCTIONS.md#target-structures-and-merge)
- [Direct runtime-change semantics](RECONFIGURATION_1.1.md)
- [Historical/current-source audit](CONFIGURATION_1.0.md)
- [Configuration directory disposition](../../config/README.md)

## Target

```text
one process invocation
→ bounded typed file + selected env/CLI overrides
→ concrete DaemonSettings
→ validate final values
→ construct owners/handlers
→ factual status/readiness
```

Use only fields consumed by the current application. Keep paths as OS values. Bound the file before decoding.
Unknown fields fail. Optional components are configurable only when selected and implemented.

The old section registry, example catalogue, fingerprints, action plans, receipts, wave ownership and accepted
readiness flags are not a supported schema. The checked-in example is intentionally a parser-compatible no-op
until #333 replaces parser, settings and consumers atomically.
