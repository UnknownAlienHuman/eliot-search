# Working on search-config

Read the [architecture](../../docs/architecture/README.md) and
[implementation](../../docs/implementation/README.md). This package should make settings easy to
understand and use: typed values, defaults, explicit overrides and validation of real fields.

Change the daemon capture, construction and actual setting consumers together. Keep OS/file capture
at the application edge and dependency effects with their existing owners. A package boundary does
not require generic descriptors, section hashes, receipt enums or a reload framework.

Startup constructs the application from accepted settings. It does not apply a change to an already
running defaults-only application. Do not satisfy `try_activate` with manufactured receipt kinds.
Only implement hot reload for a concrete supported change; other settings can require restart.

Use the existing locked TOML/Serde implementation directly for typed decoding; the current event
adapter and schema are described in [TOML_PROFILE_2.md](TOML_PROFILE_2.md), not frozen requirements.
Remove replaced parsing/canonicalization machinery with its callers. No legacy decoder or migration
platform is required for pre-release configuration.

Check meaningful behavior when implementing: precedence reaches the real consumer, invalid settings
fail before construction, paths remain OS strings, file reads are actually bounded, and status reports
the running settings. A fingerprint, source-line quota or authored fixture does not prove this.
