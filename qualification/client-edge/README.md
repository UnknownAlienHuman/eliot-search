# W8 client-edge qualification

This directory is the P14/G4 evidence contract for the generic local provider edge and optional
compatibility/export profiles.

- [`W8_QUALIFICATION.md`](W8_QUALIFICATION.md) — execution sequence, evidence and stop conditions.
- [`baseline.toml`](baseline.toml) — locked generic-edge and optional-profile invariants.
- [`probes.toml`](probes.toml) — machine-readable probe inventory; all results initially unavailable.
- [`gate-map.toml`](gate-map.toml) — mapping from Architecture G4 evidence IDs to probe IDs.

Generic provider qualification is required for G4. ELIOT and Research profiles remain disabled and
are required only when explicitly enabled. Their absence is not a standalone baseline failure.

No runtime evidence is present in this directory. A future result is valid only with exact code/API/
config/fixture/platform identities, immutable raw output and independent review.
