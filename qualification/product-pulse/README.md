# P15 Product Pulse qualification

This directory is the machine-readable G5 acceptance contract for the exact ELIOT Search baseline on
Windows x64.

- [`W9_QUALIFICATION.md`](W9_QUALIFICATION.md) — execution order, evidence rules and stop conditions.
- [`baseline.toml`](baseline.toml) — frozen-run and hard-invariant template; currently not executed.
- [`corpus.toml`](corpus.toml) — mandatory S35 case inventory; currently unmaterialized.
- [`metrics.toml`](metrics.toml) — metric definitions and Architecture S30.2 candidate targets.
- [`probes.toml`](probes.toml) — sixty mandatory probes, all initially `UNAVAILABLE`.
- [`gate-map.toml`](gate-map.toml) — exact mapping to the six existing G5 evidence IDs.
- [`fixture-owners.toml`](fixture-owners.toml) — semantic fixture owners and accepted-reference slots.

No file here is an acceptance receipt. Baselines, Windows environment, quality policy and corpus fixture
refs remain unselected until an integration-owner ticket freezes them before candidate results are
observed.
