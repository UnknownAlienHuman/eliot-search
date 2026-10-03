# ELIOT Search — execution index

Audited code baseline: `a5abdf7ef0cb9d691000759494fd8829b2ba0b60`. Coordinator: [#97](https://github.com/UnknownAlienHuman/eliot-search/pull/97).

**43 task drafts, all NOT_STARTED.** These PRs contain bounded assignments, not completed implementations. Main is unchanged. [Audit](AUDIT.md), [execution/acceptance contract](PROGRAM.md), [machine dependency map](plan.json).

## Start and scheduling

Begin T01 → T02 → T03 → T04. T01 reconciles current registries and stale issues; it does not fabricate accepted handoffs. The documented M0 bootstrap allows reviewed fixes for existing build blockers, but only T04 can establish the executed baseline.

After T04, independent ready work includes T05/T07, T09, T22 and T41. T05 and T07 share the daemon lock; T22 and T41 share root-manifest/registry locks. They cannot run simultaneously when those locks overlap. Read `depends_on`, not just milestone number. No batch launch of 43 writers.

Before taking a task, refresh its branch from accepted main, resolve moved/proposed paths using T01/T02, acquire every actual touched-package lock, and materialize the accepted dependency handoffs. The locks in plan.json are a minimum inventory, not issued write leases. All remaining task packets name their predecessor IDs and exact scoped acceptance scenarios.

## Required vertical checkpoints

- [T17 / #114](https://github.com/UnknownAlienHuman/eliot-search/pull/114): primary durable DIRECT with verified retained bytes/preparation and restart.
- [T30 / #127](https://github.com/UnknownAlienHuman/eliot-search/pull/127): actual CLI → daemon → CAS/redb → Qdrant → source-validated result, including recovery.
- [T43 / #140](https://github.com/UnknownAlienHuman/eliot-search/pull/140): installed Windows distribution and all baseline lifecycle/quality/security acceptance; transitively depends on the other 42 tasks.

## Task PRs

Each PR adds one unique task file on its own planning branch. Open that PR for the packet. Keep it draft until code, required executed tests and independent review are present; never merge a packet-only draft as implementation completion.

### M0 — Qualified baseline

| Task / PR | Required outcome | Prerequisites | Owner |
|---|---|---|---|
| [T01 / #98](https://github.com/UnknownAlienHuman/eliot-search/pull/98) | Reconcile execution authority, package inventory and stale assignments | Entry task | integration |
| [T02 / #99](https://github.com/UnknownAlienHuman/eliot-search/pull/99) | Isolate legacy entrypoints and restore micromodular ownership | T01 (#98) | integration |
| [T03 / #100](https://github.com/UnknownAlienHuman/eliot-search/pull/100) | Replace nightly Windows file identity calls with stable handle observations | T02 (#99) | integration |
| [T04 / #101](https://github.com/UnknownAlienHuman/eliot-search/pull/101) | Establish an executed reproducible build and regression gate | T03 (#100) | integration |

### M1 — Safe owner and runtime

| Task / PR | Required outcome | Prerequisites | Owner |
|---|---|---|---|
| [T05 / #102](https://github.com/UnknownAlienHuman/eliot-search/pull/102) | Quarantine the primary runtime after uncertain catalog effects | T04 (#101) | integration |
| [T06 / #103](https://github.com/UnknownAlienHuman/eliot-search/pull/103) | Bound protocol framing, child lifetime and real disconnect recovery | T05 (#102) | integration |
| [T07 / #104](https://github.com/UnknownAlienHuman/eliot-search/pull/104) | Connect final-handle containment and source-read security | T04 (#101) | integration |
| [T08 / #105](https://github.com/UnknownAlienHuman/eliot-search/pull/105) | Compose one durable installation and runtime owner | T05 (#102), T07 (#104) | integration |

### M2 — Authoritative redb control

| Task / PR | Required outcome | Prerequisites | Owner |
|---|---|---|---|
| [T09 / #106](https://github.com/UnknownAlienHuman/eliot-search/pull/106) | Make the persistent redb adapter bounded and port-complete | T04 (#101) | package:search-control-redb |
| [T10 / #107](https://github.com/UnknownAlienHuman/eliot-search/pull/107) | Decode and verify legacy control migration without switching authority | T08 (#105), T09 (#106) | integration |
| [T11 / #108](https://github.com/UnknownAlienHuman/eliot-search/pull/108) | Switch the primary daemon to atomic redb control state | T10 (#107) | integration |
| [T12 / #109](https://github.com/UnknownAlienHuman/eliot-search/pull/109) | Apply real configuration snapshots and truthful capability readiness | T11 (#108) | integration |

### M3 — Durable DIRECT

| Task / PR | Required outcome | Prerequisites | Owner |
|---|---|---|---|
| [T13 / #110](https://github.com/UnknownAlienHuman/eliot-search/pull/110) | Connect canonical admission, identity and source registry to ingestion | T07 (#104), T11 (#108), T12 (#109) | integration |
| [T14 / #111](https://github.com/UnknownAlienHuman/eliot-search/pull/111) | Bind immutable revision CAS to full residency domains | T13 (#110) | integration |
| [T15 / #112](https://github.com/UnknownAlienHuman/eliot-search/pull/112) | Close deterministic representation profiles and coordinate maps | T14 (#111) | package:search-materializer |
| [T16 / #113](https://github.com/UnknownAlienHuman/eliot-search/pull/113) | Persist canonical preparation and unit manifests | T15 (#112) | integration |
| [T17 / #114](https://github.com/UnknownAlienHuman/eliot-search/pull/114) | Route DIRECT through the complete canonical durable spine | T16 (#113), T05 (#102) | integration |

### M4 — Authorized provider

| Task / PR | Required outcome | Prerequisites | Owner |
|---|---|---|---|
| [T18 / #115](https://github.com/UnknownAlienHuman/eliot-search/pull/115) | Compose OS secret leases and replay-resistant local pairing | T08 (#105), T12 (#109) | integration |
| [T19 / #116](https://github.com/UnknownAlienHuman/eliot-search/pull/116) | Expose the canonical provider protocol through daemon and CLI | T06 (#103), T17 (#114), T18 (#115) | integration |
| [T20 / #117](https://github.com/UnknownAlienHuman/eliot-search/pull/117) | Bind live grants and security barriers before retrieval and IDF | T13 (#110), T19 (#116) | integration |
| [T21 / #118](https://github.com/UnknownAlienHuman/eliot-search/pull/118) | Use canonical live-authorized handles and continuations | T20 (#117) | integration |

### M5 — Live Qdrant indexed spine

| Task / PR | Required outcome | Prerequisites | Owner |
|---|---|---|---|
| [T22 / #119](https://github.com/UnknownAlienHuman/eliot-search/pull/119) | Qualify one exact Qdrant server-client-artifact profile | T04 (#101) | integration |
| [T23 / #120](https://github.com/UnknownAlienHuman/eliot-search/pull/120) | Run Qdrant through a real owned process supervisor | T08 (#105), T18 (#115), T22 (#119) | integration |
| [T24 / #121](https://github.com/UnknownAlienHuman/eliot-search/pull/121) | Implement the bounded real Qdrant data-plane adapter | T20 (#117), T23 (#120) | package:search-qdrant-bridge |
| [T25 / #122](https://github.com/UnknownAlienHuman/eliot-search/pull/122) | Wire deterministic lexical encoding and scoring-document accounting | T16 (#113), T22 (#119) | package:search-lexical |
| [T26 / #123](https://github.com/UnknownAlienHuman/eliot-search/pull/123) | Build real membership-scoped projection and point manifests | T13 (#110), T16 (#113), T25 (#122) | integration |
| [T27 / #124](https://github.com/UnknownAlienHuman/eliot-search/pull/124) | Compose serialized epoch publication with crash recovery | T11 (#108), T24 (#121), T26 (#123) | integration |
| [T28 / #125](https://github.com/UnknownAlienHuman/eliot-search/pull/125) | Connect indexed retrieval, source validation and result projection | T19 (#116), T21 (#118), T27 (#124) | integration |
| [T29 / #126](https://github.com/UnknownAlienHuman/eliot-search/pull/126) | Implement route rebuild, epoch pins and safe index reclamation | T28 (#125) | integration |
| [T30 / #127](https://github.com/UnknownAlienHuman/eliot-search/pull/127) | Prove the live Rust-Qdrant product spine end to end | T29 (#126) | integration |

### M6 — Currentness and baseline query breadth

| Task / PR | Required outcome | Prerequisites | Owner |
|---|---|---|---|
| [T31 / #128](https://github.com/UnknownAlienHuman/eliot-search/pull/128) | Manage registered roots live and reconcile current workspace truth | T17 (#114), T19 (#116), T20 (#117) | integration |
| [T32 / #129](https://github.com/UnknownAlienHuman/eliot-search/pull/129) | Add exact no-execute Git source acquisition and lineage | T13 (#110), T14 (#111), T31 (#128) | integration |
| [T33 / #130](https://github.com/UnknownAlienHuman/eliot-search/pull/130) | Integrate authenticated ephemeral editor overlays | T21 (#118), T28 (#125), T31 (#128) | integration |
| [T34 / #131](https://github.com/UnknownAlienHuman/eliot-search/pull/131) | Expose executable frozen-denominator exact proof plans | T21 (#118), T28 (#125), T31 (#128) | integration |
| [T35 / #132](https://github.com/UnknownAlienHuman/eliot-search/pull/132) | Connect structural code enrichment to exact retained units | T16 (#113), T31 (#128) | integration |
| [T36 / #133](https://github.com/UnknownAlienHuman/eliot-search/pull/133) | Execute baseline recipes, subject resolution and comparisons | T28 (#125), T34 (#131), T35 (#132) | integration |

### M7 — Lifecycle and Windows release

| Task / PR | Required outcome | Prerequisites | Owner |
|---|---|---|---|
| [T37 / #134](https://github.com/UnknownAlienHuman/eliot-search/pull/134) | Integrate ordinary retention, pinned evidence and safe CAS collection | T21 (#118), T29 (#126), T31 (#128) | integration |
| [T38 / #135](https://github.com/UnknownAlienHuman/eliot-search/pull/135) | Execute security purge barriers across every storage and result plane | T20 (#117), T27 (#124), T37 (#134), T33 (#130) | integration |
| [T39 / #136](https://github.com/UnknownAlienHuman/eliot-search/pull/136) | Implement bounded restore, key migration and ownership cutover | T11 (#108), T14 (#111), T38 (#135) | integration |
| [T40 / #137](https://github.com/UnknownAlienHuman/eliot-search/pull/137) | Enforce content-minimized diagnostics and measured resource budgets | T30 (#127), T36 (#133), T38 (#135) | integration |
| [T41 / #138](https://github.com/UnknownAlienHuman/eliot-search/pull/138) | Replace required Python validation with Rust Cargo tooling | T01 (#98), T04 (#101) | integration |
| [T42 / #139](https://github.com/UnknownAlienHuman/eliot-search/pull/139) | Make optional workers and leaf adapters honestly unavailable by default | T19 (#116), T36 (#133) | integration |
| [T43 / #140](https://github.com/UnknownAlienHuman/eliot-search/pull/140) | Ship and verify one reproducible Windows baseline distribution | T02 (#99), T30 (#127), T31 (#128), T32 (#129), T33 (#130), T34 (#131), T35 (#132), T36 (#133), T37 (#134), T38 (#135), T39 (#136), T40 (#137), T41 (#138), T42 (#139) | integration |

## Audit coverage

| Finding | Assigned tasks |
|---|---|
| A01 | T03 (#100), T04 (#101) |
| A02 | T01 (#98) |
| A03 | T01 (#98) |
| A04 | T02 (#99) |
| A05 | T02 (#99), T08 (#105) |
| A06 | T05 (#102), T10 (#107), T11 (#108) |
| A07 | T04 (#101), T05 (#102), T10 (#107), T11 (#108), T37 (#134) |
| A08 | T03 (#100), T07 (#104) |
| A09 | T09 (#106), T10 (#107), T11 (#108), T12 (#109) |
| A10 | T12 (#109) |
| A11 | T13 (#110) |
| A12 | T14 (#111) |
| A13 | T15 (#112), T16 (#113), T17 (#114) |
| A14 | T06 (#103) |
| A15 | T18 (#115), T19 (#116) |
| A16 | T20 (#117) |
| A17 | T21 (#118) |
| A18 | T22 (#119), T23 (#120), T24 (#121) |
| A19 | T22 (#119), T24 (#121), T25 (#122) |
| A20 | T25 (#122), T26 (#123), T27 (#124), T28 (#125), T29 (#126) |
| A21 | T30 (#127) |
| A22 | T31 (#128) |
| A23 | T32 (#129) |
| A24 | T33 (#130) |
| A25 | T34 (#131) |
| A26 | T35 (#132), T36 (#133) |
| A27 | T37 (#134), T38 (#135) |
| A28 | T39 (#136) |
| A29 | T40 (#137) |
| A30 | T41 (#138) |
| A31 | T42 (#139) |
| A32 | T43 (#140) |

All 32 findings have assigned tasks. Every one of the 43 tasks is reachable from the entry task and included in final release closure. This is a planning-graph check, not product qualification. No compiler/native/runtime tests ran during this audit; exact-head execution belongs to the implementation gates.

### Merge discipline

Implementation → named positive/negative/fault tests on exact head → independent review → integration merge and readback → release dependencies. Keep main unchanged during task planning. Preserve already-landed safety regressions. No automatic workflow triggers, false receipts or in-memory substitutes for real native/backend evidence.
