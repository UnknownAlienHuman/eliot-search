# W9 Product Pulse settings 1.0

Machine schema: [`../../config/w9-product-pulse.toml`](../../config/w9-product-pulse.toml).

These are evaluation-run settings, not production Search settings. They cannot alter production
contracts, ranking, grants, source admission, logging floors or runtime capability state.

## Modes

- `LOCKED` — correctness, fairness, privacy or gate invariant; any override is rejected.
- `TUNABLE` — finite run-control value applied only to a new frozen run manifest.
- `QUALIFIED_REF` — immutable accepted environment, baseline, policy or artifact-store reference.

No file/environment/CLI value can change a locked field. `UNSELECTED` qualified refs block execution.

## Tunable values

Only warm-up/measured attempt counts, finite case/block deadlines and resource-sampling interval are
runtime tunables. They apply to the next run and become part of its digest. Mid-run changes are rejected.

The default of thirty measured observations is the minimum required by the metric registry for p95.
An integration ticket may select more observations within the published bound before candidate results
are visible.

## Locked fairness rules

- randomized paired A/B/C order with captured seed;
- identical declared corpus/scope/view and no network;
- cold/warm lanes and preparation costs separate;
- all required cases and at least eight independent lineages;
- mutable fixtures and hidden case removal forbidden;
- oracle invisible to production and baseline drivers.

## Locked evidence rules

- immutable raw output and independent review required for PASS;
- failed, cancelled, timed-out and unavailable observations preserved;
- receipts append-only;
- prose-only evidence rejected;
- source/query/secret/token/absolute-path content forbidden in ordinary reports.

## Locked safety rules

Stale leakage, access leakage, secret/content leakage and false complete-negative tolerance are all zero.
Hard blockers cannot be averaged or weighted away. Optional profiles remain disabled during baseline
acceptance.

## Verdict rules

Criteria are frozen before candidate results. Self-review, self-acceptance, missing-evidence PASS,
unit-test-only PASS and compilation-only PASS are impossible. Product acceptance and W10 authorization
are receipts, not configuration booleans.

A new global production configuration snapshot is not created by an evaluation run. Production
observability remains governed by `config/sections/observability.md`; the harness may only verify those
floors.

## Required settings tests

- every field has one mode/type/default and finite bounds where tunable;
- qualified refs default `UNSELECTED` and block run freeze;
- locked fields reject file/environment/CLI override;
- mid-run tuning rejected;
- measured iterations below thirty rejected for required p95 report;
- zero-tolerance and no-network floors cannot change;
- optional profile activation and product acceptance by config impossible;
- canonical settings digest is independent of source-file ordering;
- redacted settings view contains no secret/path/source/query data.
