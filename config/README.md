# ELIOT Search configuration

Configuration supplies values consumed by the running application. Start with the
[implementation guide](../docs/implementation/README.md) and
[config change map](../crates/search-config/FUNCTIONS.md).

## Current code

[search-config](../crates/search-config/README.md) already implements parsing, layering and generic
registry/reconfiguration machinery. The [current decoder](../crates/search-config/TOML_PROFILE_2.md)
accepts schema2 and a restricted TOML profile. The
[daemon registry](../bins/eliot-searchd/src/config_composition/kernel/registry.rs) identifies its current
fields; registration alone does not demonstrate that a runtime consumer applies them.

[sections.toml](sections.toml), the [section references](sections/), the
[example TOML](eliot-search.example.toml) and W5–W10 files include historical proposals and unconnected
fields. The example is not a complete tested configuration for the current daemon. Check the real
registry and consumer before treating a field as supported.

## Implementation direction

Capture one invocation, decode/merge supported overrides into typed settings, validate their actual
values and construct the selected owners. Status reads those running settings. Use the existing locked
TOML/Serde decoder; remove replaced generic trees, provenance hashes and per-section receipt planning.

A supported runtime update calls its existing owner; otherwise require restart. Index profile changes
use publication. Neither startup nor ordinary settings require qualification gates, compulsory secret
lifecycle or a separate config transaction engine. The
[configuration guide](../docs/config/README.md) distinguishes this target from historical formats.
