# search-config

Typed settings for one local application. Defaults, selected file values and explicit environment/CLI
overrides produce the settings used to construct the runtime.

Read the [implementation guide](../../docs/implementation/README.md) and
[concrete change map](FUNCTIONS.md). This describes the refactoring target; the Rust implementation
still contains the registry/fingerprint/reconfiguration machinery below.

## Current code and consumer

`parse_document` uses `toml_parser` events to build generic `ConfigDocument` values. Registry/merge,
section projection, canonical section/snapshot hashing, diff and receipt-based action planning are
separate modules. The daemon's
[config composition](../../bins/eliot-searchd/src/config_composition/kernel/process.rs) captures inputs
and assembles these values. Its `effective_from_process` compares a startup candidate with defaults
and calls `try_activate` with no receipts. A permitted `query.limit=11` therefore gets
`DAEMON_CONFIG_PARTIAL_REFUSED` instead of constructing an application with that value.

The [current file profile](TOML_PROFILE_2.md) accepts schema2 and a restricted TOML subset.
The historical schema1 reference and reload plans do not constrain the replacement.

## Implementation result

Use concrete setting structs for consumed fields, a small override merge and field validation. Decode
with the already locked `toml` + Serde API. No dynamic application-level TOML tree or generic registry
is needed for a fixed settings model. Preserve useful defaults, supported override precedence, clear
errors and real size limits. Do not accept documented fields that no runtime owner consumes.

Capture once, validate once, construct owners with the accepted settings and let status read that
running state. Remove configuration provenance hashes, per-section receipts and mandatory secret
settings from ordinary local startup. Optional credential use belongs to the selected dependency.

Runtime changes call an existing owner where supported. A query default can affect new requests; a
static process option may require restart; an indexed profile change uses the existing generation and
publication path. These cases do not require a second general transaction/recovery engine for config.
