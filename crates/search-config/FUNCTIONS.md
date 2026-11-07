# Settings consumed by the real command

The unused generic config Rust package, registry, fingerprints, manifest and exclusive fixtures are removed.
Native argument/settings capture lives beside the actual daemon/CLI constructors.

## Foundation fields shipped with root work

Only data_root is selected from bounded file/environment/CLI root settings. Keep diagnostics bounded
by the actual fixed output limit. Add settings only with a constructed consumer; no dormant snapshots.

## Target capture

Capture startup cwd and native args once. Resolve relative file paths against the configuration parent;
CLI/environment paths use startup cwd. Preserve native PathBuf/OsString inputs.
Read the selected file once with the fixed bootstrap byte cap, then use the maintained typed TOML/Serde parser.

## Layer semantics

Defaults < file < environment < CLI. Omission inherits; explicit empty/false values remain explicit.
Validate malformed lower layers even when overridden. Reject unknown keys, duplicate definitions,
wrong types and unsupported ranges before effects. Ignore unrelated environment entries.
Use only the selected ELIOT_SEARCH__ namespace and reject unknown names within it.

## Diagnostics and secrets

Report bounded field/error classes, never input values, config excerpts or keys.
An optional existing Qdrant API key belongs to its actual bridge consumer.

## Status and runtime changes

Report actual constructed owner state. Baseline settings are startup-only.
No generic hot-reload, receipt-gated activation, second parser or fingerprint-heavy status platform.
