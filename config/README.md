# ELIOT Search configuration

Configuration is implementation-scaffolded, not implemented.

- [`sections.toml`](sections.toml) — machine registry of section owner, earliest wave, minimum action,
  secret policy and bounded packet path.
- [`sections/`](sections/) — one small field/validation/reload packet per capability-owned section.
- [`eliot-search.example.toml`](eliot-search.example.toml) — complete safe DIRECT-mode example.
- [`../docs/config/CONFIGURATION_1.0.md`](../docs/config/CONFIGURATION_1.0.md) — full cross-package
  configuration contract.

`search-config` owns parsing, layering, provenance, redaction, fingerprints, diffs and composite
reconfiguration planning. It owns no capability setting and performs no I/O. Each section owner owns
typed defaults, validation, digest and live-application behavior.

The example does not select a Qdrant build or lexical profile. Indexed settings remain disabled or
`UNQUALIFIED` until P05–P07 evidence is accepted. Plaintext secrets, automatic downloads/upgrades and
optional-profile self-authorization are invalid.
