# Local configuration

- [`sections.toml`](sections.toml) is the integration-owned section/owner/reload registry.
- [`eliot-search.example.toml`](eliot-search.example.toml) is a safe DIRECT-profile example, not a
  generated effective config and not evidence that runtime parsing exists.
- Field-level semantics are in [`../docs/config/CONFIGURATION_1.0.md`](../docs/config/CONFIGURATION_1.0.md).

Production configuration is UTF-8 TOML with duplicate and unknown load-bearing keys rejected. Plaintext
secrets are forbidden; use opaque OS-bound secret references. Package writers own only their declared
section and do not edit this directory.
