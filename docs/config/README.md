# Configuration

The [architecture](../architecture/README.md) explains the product; the
[implementation guide](../implementation/README.md) defines the current refactoring target.
Configuration supplies values that actual runtime components use.

## Read for implementation

- [Config function/change map](../../crates/search-config/FUNCTIONS.md): real startup path, refusal bug,
  typed settings replacement and capture fixes.
- [TOML decoder](../../crates/search-config/TOML_PROFILE_2.md): existing grammar, locked donor API and
  its actual allocation tradeoff.
- [Daemon registry](../../bins/eliot-searchd/src/config_composition/kernel/registry.rs): fields currently
  registered in Rust. Registration alone does not prove a runtime consumer.
- [Runtime changes](RECONFIGURATION_1.1.md): startup construction versus supported updates.

Capture once, decode/merge, validate the consumed fields and construct the application. Startup does
not require receipts for differences from defaults. Status reads the running settings. A configuration
file is not an instruction to create every optional subsystem.

## Historical design references

[CONFIGURATION_1.0.md](CONFIGURATION_1.0.md), the W5–W10 settings packets,
[sections.toml](../../config/sections.toml), [section packets](../../config/sections/) and
[example TOML](../../config/eliot-search.example.toml) include proposed fields and old lifecycle rules.
They are not a complete tested configuration for the current daemon. Check each field against the
real registry and consumer when updating it. Old waves, fingerprint equality, receipt requirements and
section ownership walls do not constrain the replacement.
