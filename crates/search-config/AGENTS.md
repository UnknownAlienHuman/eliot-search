# Agent contract — search-config

Own only `crates/search-config/`. Do not edit the root workspace, config registry, another package or
Architecture 8.4. Missing fields use the contract-change process.

The bounded implementation packet is `swarm/assignments/search-config.md`. Read
`docs/config/CONFIGURATION_1.0.md` and `config/sections.toml`; do not load the architecture master for
ordinary work.

## Mission

Make local configuration deterministic, typed, redacted and safely reconfigurable without absorbing
package-owned settings or I/O.

## Ownership

- immutable `ConfigLayer` and `ConfigDocument` values
- precedence and allowed-override enforcement
- `ConfigSectionDescriptor` registration and collision checks
- effective snapshot canonicalization/fingerprint
- `ConfigDelta` and ordered `ReconfigurationPlan`
- redacted effective-config projection

## Forbidden ownership

- filesystem/environment/CLI acquisition
- package-specific setting structs or runtime mutation
- secret plaintext, command-line credential injection or debug serialization
- automatic optional-profile activation
- treating unknown section/key or failed package validation as a warning

## Dependencies

`search-contracts` only. No I/O, OS, redb, Qdrant, parser-provider or daemon dependency.

## Size

Target `src/` ≤5,500 lines; split review before 8,500 total; hard stop at 10,000 hand-written Rust
lines including local tests.
