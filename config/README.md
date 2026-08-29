# ELIOT Search configuration

Configuration is implementation-scaffolded, not implemented.

- [`sections.toml`](sections.toml) — machine registry of section owner, earliest wave, minimum action,
  secret policy and bounded packet path.
- [`sections/`](sections/) — one small field/validation/reload packet per capability-owned section.
- [`eliot-search.example.toml`](eliot-search.example.toml) — complete safe DIRECT-mode example.
- [`w5-current.toml`](w5-current.toml) — locked W5 currentness, transient-overlay and Rust parser-profile
  settings; it is a stage qualification schema, not implementation authorization.
- [`../docs/config/CONFIGURATION_1.0.md`](../docs/config/CONFIGURATION_1.0.md) — full cross-package
  configuration contract.

`search-config` owns parsing, layering, provenance, redaction, fingerprints, diffs and composite
reconfiguration planning. It owns no capability setting and performs no I/O. Each section owner owns
typed defaults, validation, digest and live-application behavior.

Stage-specific `w*-*.toml` files add locked qualification and bounded orchestration settings. They do not
create new authority, select artifacts/providers or permit a future wave to start.

The example does not select a Qdrant build, lexical profile or optional provider. Indexed settings remain
disabled or `UNQUALIFIED` until their evidence is accepted. Plaintext secrets, automatic downloads/
upgrades, currentness weakening and optional-profile self-authorization are invalid.
