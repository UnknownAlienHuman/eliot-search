# search-eval

**C29 — Content-minimized telemetry, qualification and Product Pulse.**

**Status:** package boundary and complete W4/W9 function contract; behavior and acceptance evidence are
intentionally unimplemented.

## Owns

- control-corpus, baseline, run, metric and evidence schemas;
- deterministic paired A/B/C aggregation;
- latency/resource/fault/protocol/security reports;
- source-admission and content-minimization audits;
- Product Pulse hard-blocker classification, verdict and immutable receipt.

## Does not own

- production query/ranking/source/index/lifecycle behavior;
- raw source, unsaved buffers, query text, secrets, tokens or absolute paths in ordinary telemetry;
- hidden training/learning or oracle feedback;
- cross-package fault execution or another package's fixtures;
- gate self-acceptance;
- production packages depending on eval.

- **Delivery:** W4/P08 baseline schemas; W9/P15 acceptance.
- **Soft `src/` target:** 7,500 hand-written lines.
- **Split review:** before 8,500 total hand-written lines.
- **Hard stop:** 10,000 including package-local tests.
- **Agent instructions:** [AGENTS.md](AGENTS.md)
- **W9 cross-contract:** [`../../docs/evaluation/W9_PRODUCT_PULSE_CONTRACTS_1.0.md`](../../docs/evaluation/W9_PRODUCT_PULSE_CONTRACTS_1.0.md)
