# search-eval

**C29 — Content-minimized evaluation and candidate Product Pulse reports.**

**Status:** source is present for schemas, audits, metrics, evidence handling and reports. This is an evaluation-only package, not an accepted installed-product measurement or qualification system. Existing local fingerprints and caller-supplied evidence still require the migrations assigned below. Source presence and compilation do not establish product acceptance.

## Owns

- pure corpus, baseline, run, metric and evidence models;
- deterministic aggregation and compatible-run comparison;
- candidate latency/resource/fault/protocol/security reports;
- content-minimization and source-admission audit calculations;
- candidate Product Pulse classification and report construction.

The current implementation is exported by `src/lib.rs` from `audits`, `core`, `evidence`, `metrics` and `report`. The private `fingerprint` module is retained migration debt, not proof that all identities already use the canonical digest owner.

## Current implementation boundaries

- [#233](https://github.com/UnknownAlienHuman/eliot-search/issues/233): immutable run/query/qrel/result formats and pure evaluation.
- [#234](https://github.com/UnknownAlienHuman/eliot-search/issues/234): independently checked statement subjects, predicates and reviewer policy.
- [#240](https://github.com/UnknownAlienHuman/eliot-search/issues/240): effectful installed-product execution and retained raw measurements.
- [#137](https://github.com/UnknownAlienHuman/eliot-search/pull/137): measurement/disclosure integration program; historical tracking branch is not an implementation base.
- [#215](https://github.com/UnknownAlienHuman/eliot-search/issues/215): preregistered product qualification.

Canonical computation belongs to `search-contracts`. A caller's status, sample, digest or approval Boolean is not independently verified execution evidence.

## Does not own

- production query, ranking, source, index or lifecycle behavior;
- production telemetry services or hidden learning/oracle feedback;
- raw source, unsaved buffers, query text, secrets, tokens or private paths in ordinary reports;
- a second installed-product runner, store reader or cross-package fault executor;
- independent acceptance of its own candidate report;
- production packages depending on evaluation tooling.

## Working entrypoints

- [Package instructions](AGENTS.md) and [function contract](FUNCTIONS.md).
- [Current audit/launch entrypoint](../../docs/audit/README.md).
- [Central package status](../../docs/product/PACKAGE_STATUS.toml).
- [Evaluation contract](../../docs/evaluation/W9_PRODUCT_PULSE_CONTRACTS_1.0.md).

Historical W4/W9/P08/P15 labels trace obligations only. They do not replace the current root instructions and assigned issue, and do not authorize a controller, ticket or lease system inside Search.

Soft source target: 7,500 hand-written lines; split review before 8,500 total; hard review threshold 10,000 including local tests. Split by responsibility, not by inventing another report or acceptance authority.
