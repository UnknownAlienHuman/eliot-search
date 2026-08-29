# W8 generic client edge contracts

This directory is the bounded P14/G4 implementation projection for the local generic Search provider
edge and optional compatibility/export profiles.

- [`W8_GENERIC_CLIENT_EDGE_CONTRACTS_1.0.md`](W8_GENERIC_CLIENT_EDGE_CONTRACTS_1.0.md) — transport,
  binding, grant, capability, request/result, handle expansion and authority boundaries.
- [`manifest.toml`](manifest.toml) — machine owner/read-set inventory and current blocked status.
- [`../config/W8_CLIENT_EDGE_SETTINGS_1.0.md`](../config/W8_CLIENT_EDGE_SETTINGS_1.0.md) — locked and
  tunable W8 settings.
- [`../../qualification/client-edge/README.md`](../../qualification/client-edge/README.md) — future
  executed evidence packet; all probes are initially unavailable.

W8 introduces no new Search recipe and no client authority. The exact eleven P00 recipes remain
canonical. Optional ELIOT and Research profiles are disabled by default and are not required for the
standalone baseline.

These files do not authorize implementation. `swarm/launch-state.toml` remains the sole launch
authority, and Architecture Part I remains normative.
