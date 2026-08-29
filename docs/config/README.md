# Configuration contracts

- [`CONFIGURATION_1.0.md`](CONFIGURATION_1.0.md) — local format, precedence, section ownership, field
  bounds and lifecycle descriptions.
- [`RECONFIGURATION_1.1.md`](RECONFIGURATION_1.1.md) — authoritative correction: reload requirements are
  a composite action set, not a lossy single severity value.
- [`../../config/sections.toml`](../../config/sections.toml) — machine-readable owner, earliest wave,
  minimum action, secret policy and per-section packet.
- [`../../config/sections/`](../../config/sections/) — bounded section-owner field/API/test packets.
- [`../../config/eliot-search.example.toml`](../../config/eliot-search.example.toml) — complete safe
  DIRECT-profile example.

Where 1.0 and 1.1 conflict on `dominant_action` or total ordering, 1.1 wins. These files define
implementation inputs; they do not claim parsing or runtime reload has been implemented.
