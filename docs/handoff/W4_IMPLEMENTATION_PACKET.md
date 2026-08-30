# W4 baseline query-product implementation packet

**Stage / wave:** P08 / W4  
**Status:** `BLOCKED` until accepted W1–W3 handoffs and exact qualified DIRECT/lexical index capability.  
**Goal:** expose bounded server-owned local search recipes whose candidates are access/currentness
filtered, source-backed and rendered as compact truthful results.

## Package order

```text
accepted contracts/domain/ports + W2 source/revision spine + W3 lexical/index/publication/pins
        ↓
search-access
        ↓
search-query-planner
        ↓
search-retrieval-executor
        ↓
search-candidate-validator
        ↓
search-handles      search-result-projector      search-continuation
        ↓
search-eval baseline schemas / control corpus seams
```

Independent result/handle/eval work may start only against exact accepted direct API digests. Provider
framing remains owned by `search-provider-protocol`; daemon composition connects it after the W4 handler
set is accepted.

## One-agent package packets

| Package | Primary packet | Write scope |
|---|---|---|
| `search-access` | `crates/search-query/search-access/FUNCTIONS.md` | package only |
| `search-query-planner` | `crates/search-query/search-query-planner/FUNCTIONS.md` | package only |
| `search-retrieval-executor` | `crates/search-query/search-retrieval-executor/FUNCTIONS.md` | package only |
| `search-candidate-validator` | `crates/search-query/search-candidate-validator/FUNCTIONS.md` | package only |
| `search-handles` | `crates/search-query/search-handles/FUNCTIONS.md` | package only |
| `search-result-projector` | `crates/search-query/search-result-projector/FUNCTIONS.md` | package only |
| `search-continuation` | `crates/search-query/search-continuation/FUNCTIONS.md` | package only |
| `search-eval` | `crates/search-eval/FUNCTIONS.md` | package only |

Exact paths/write scopes are registered in `swarm/function-packets.toml`.

## Request pipeline

```text
authenticated binding + current grant + explicit requested scope
→ search-access canonical eligibility/security fence
→ search-query-planner bounded server-owned plan and coverage budget
→ search-retrieval-executor finite legs under route/epoch pins
→ search-candidate-validator exact retained-revision/source readback and live rechecks
→ search-result-projector compact result cards + source handles + typed gaps
→ optional bounded continuation/handle expansion with reauthorization
```

No client supplies raw Qdrant filters, point IDs, reusable access decisions or execution graphs.

## Core invariants

- requested scope is intersected with binding policy/current grant/current memberships/live deny/purge;
- access/currentness/shadow/purge filters apply before retrieval, IDF, fusion, counts and traces;
- inaccessible or newly denied populations cannot influence authorized scores/order/results;
- query plans have finite legs/candidates/reads/bytes/deadlines and zero never means unlimited;
- planner owns recipe expansion, route/profile selection and explicit coverage semantics;
- executor returns bounded nominations and typed partial leg outcomes, never source evidence;
- candidate validator reopens exact retained source revision and rechecks authorization/currentness before
  readback and emission;
- Qdrant payload/snippet/score is not evidence;
- possession of handle or continuation never grants access;
- ordinary handles/continuations are bounded, opaque, binding-scoped and ephemeral unless a separately
  accepted durable contract applies;
- result cards preserve freshness, assurance, ambiguity, denominator kind and gaps;
- partial/degraded/incomplete results are never rendered as complete success;
- cancellation/disconnect releases request-local work, pins, handles, windows and quotas;
- hot read/query admission creates no durable control row.

## Required W4 evidence

- grant/binding/membership/source-view intersection and foreign-scope non-disclosure;
- pre-candidate filtered-IDF access/currentness noninterference;
- bounded plan canonicalization and unknown recipe/field fail-closed;
- lane priority, finite queues, saturation, deadlines and cancellation;
- deterministic fusion/tie-break/leg failure and whole-influenced-leg contamination;
- exact source-revision readback, stale payload and live-head mismatch rejection;
- restrictive change at planning/retrieval/readback/emission/expansion checkpoints;
- compact result/card/source-handle redaction and coverage truthfulness;
- handle opacity, TTL/quota, live reauthorization and unsaved/durable restrictions;
- continuation pin/TTL/restart/disconnect semantics and no raw Qdrant cursor exposure;
- protocol progress/terminal ordering and one terminal response;
- baseline evaluation/control-corpus schemas without production dependency reversal;
- DIRECT/LEXICAL capability degradation and W4 qualification probes.

## Hard stops

- raw client Qdrant filter/point/cursor or client-owned execution plan;
- access filtering after candidate generation or IDF;
- Qdrant payload accepted as source truth;
- indexed top-k used as exact-proof denominator;
- handle/continuation possession bypasses current authorization;
- stale source emitted because exact revision/currentness readback is unavailable;
- partial leg/coverage gap omitted or rendered complete;
- unbounded queue, candidate set, source read, expansion or result materialization;
- hot reads writing redb/idempotency/query history;
- package duplicates provider protocol, source storage or another query owner;
- W5 currentness/overlay, W6 exact proof/comparison or W7 lifecycle behavior implemented locally.

## Handoff

Each package publishes exact commit/API/config/profile digest, operation/error inventory, deterministic/
property/fault/security tests, unavailable qualification checks and line count. Integration accepts W4
only after the generic provider path can run the closed baseline recipes with truthful bounded results,
exact source validation and access/currentness noninterference. Compilation or a successful Qdrant query
is not a W4/G2 receipt.
