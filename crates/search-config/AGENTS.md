# Agent contract — search-config

Own only `crates/search-config/`. Do not edit the root workspace, config registry, another package or
Architecture 8.4. Missing fields use the contract-change process.

The bounded implementation packet is `swarm/assignments/search-config.md`. Read the current FILE
contract in `TOML_PROFILE_2.md` and the section registry in `config/sections.toml` before ordinary
work. `docs/config/CONFIGURATION_1.0.md` is a historical schema-1 format reference, not current
parser authority.

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
- the bounded `eliot-config-toml@2` FILE decoder into the shared typed document builder

## Forbidden ownership

- filesystem/environment/CLI acquisition
- package-specific setting structs or runtime mutation
- secret plaintext, command-line credential injection or debug serialization
- automatic optional-profile activation
- treating unknown section/key or failed package validation as a warning
- a second handwritten grammar, generic dynamic TOML product tree or configuration framework

## Dependencies

`search-contracts` plus the exact private `toml_parser` event/decoder edge recorded in
`Cargo.toml` and `TOML_PROFILE_2.md`. Do not add high-level `toml`, Serde configuration trees,
parser-provider discovery, I/O, OS, redb, Qdrant or daemon dependencies.

## Size

Target `src/` ≤5,500 lines; split review before 8,500 total; hard stop at 10,000 hand-written Rust
lines including local tests.
