# ELIOT Search — единый актуальный аудит проекта

**Дата актуализации:** 2026-10-09  
**Репозиторий:** `UnknownAlienHuman/eliot-search`  
**Аудированный `main`:** `3ccef79a6850a9fd4caa7941c40e4a331cf4e0dc`  
**Координатор:** PR `#97`  
**Статус:** большой объём substantive source существует; каноническая установленная Windows-вертикаль ещё не интегрирована и не квалифицирована.

> Этот файл заменяет разрозненные audit-файлы как основной вход для человека и агента. Технические findings F01–F55 ведутся в полном downloadable audit artifact; этот repository copy фиксирует текущий диагноз, граф владельцев и исполнение.

---

## 1. Executive conclusion

ELIOT Search — уже не scaffold и не пустой набор crates. В проекте существуют крупные реализации source admission/identity/registry, safe-reader, retained revisions, materialization/unitization, DIRECT search, access/query/handles, Qdrant bridge/supervisor, publication, retention/purge/restore и optional adapters.

Продукт всё ещё не завершён по одной причине: **реальные компоненты не сведены в одну каноническую вертикаль authority → immutable source truth → preparation → projection → Qdrant → live authorization → exact source readback → public result**.

Корневые проблемы:

1. Несколько поколений implementation contracts и legacy paths одновременно присутствуют в source.
2. Authority часто подменяется caller-shaped digest/receipt/boolean или process-local state.
3. Часть boundedness проверяется после allocation/collect/read.
4. Query/lifecycle paths используют разные currentness/security semantics.
5. Многие mature low-level задачи были реализованы вручную вместо узкого donor reuse.
6. Большой исторический backlog заставлял агентов повторно «реализовывать» уже существующие пакеты.
7. Qualification evidence остаётся историческим, частичным или отсутствует на текущем exact head.

Главный selected graph:

```text
#237 canonical values/digests
→ #207 exact S11 point identity
→ #209 authoritative complete projection manifest
→ #210 immutable generation publication/recovery
→ #200 selected bounded Qdrant bridge
→ #125/#127 live product composition
```

Продуктовый слой:

```text
#218 immutable corpus/portfolio/scope authority
→ #213 retrieve_evidence@1 + orient_scope@1
→ #221 deterministic accepted ranking profile
→ #219 optional thin MCP adapter last
```

---

## 2. What is already applied in GitHub

### 2.1. Retired legacy issue wave

- `#48` closed as `completed` after direct normative-registry verification.
- `#51–#89` closed with `state_reason=not_planned` and individualized successor comments.
- This means **SUPERSEDED**, not implemented or qualified.
- No issue from the early P00/P01/P02/W0/W1/W2 wave remains open.

### 2.2. Documentation/status cutover — PR #222

The branch contains:

- `docs/product/PACKAGE_STATUS.toml`, schema v3;
- `docs/product/IMPLEMENTATION_STATUS.md`;
- `docs/audit/BACKLOG_STATUS_RECONCILIATION_2026-10-08.md`;
- corrected `docs/handoff/CONTRACT_48_RECONCILIATION.md`;
- 26 active README status corrections.

`PACKAGE_STATUS.toml` separates:

```text
contract_defined
source_present
product_path
check_clippy_evidence
live_native_qualified
public_capability_enabled
disposition
execution_chain
consumers
optional_followups
```

`SOURCE_PRESENT` never implies `PRODUCT_PATH_INTEGRATED`, `LIVE_NATIVE_QUALIFIED` or `PUBLIC_CAPABILITY_ENABLED`.

### 2.3. Tooling guard assigned to #138

`#138` owns a future `cargo metadata --locked --offline --no-deps` guard that must compare the Cargo workspace with `PACKAGE_STATUS.toml` and reject missing/extra/duplicate rows, unknown closed values, malformed task references, semantic contradictions, stale scaffold wording and parser/metadata failure represented as an empty successful inventory.

### 2.4. Donor-first task graph

Current donor tasks `#223–#245` cover Tree-sitter, SCIP, Ctags, Markdown/JATS, isolated document providers, checkpointed research connectors, deterministic orientation/evaluation, in-toto/DSSE, canonical client/encoding/config/currentness/measurement/package/backup/restore/migration boundaries.

---

## 3. Current stage by subsystem

| Subsystem | Current source state | Main blocker | Selected owner |
|---|---|---|---|
| Canonical values/digests | Multiple real and fake/copy implementations coexist | one canonical owner and profile migration | `#237` |
| CLI/provider client | Real client/session code exists inside CLI; duplicate seams remain | extract one library, one parser/JSON/native edge | `#235 → #116` |
| Configuration | Typed semantic engine exists; handwritten TOML/SHA and synthetic readiness remain | donor parser + durable effect/readback | `#238 → #109` |
| Runtime/data-root | owner kernels and guards exist | one authority across redb/source/Qdrant/provider and crash recovery | `#100/#104/#105` |
| Source admission | large kernel exists | pre-admit before bytes, honest glob semantics, native location/root proof | `#241 → #110` |
| Source currentness | registration/sync/reconcile source exists | watcher gaps + coherent final all-root revalidation | `#239 → #128 → #218` |
| Revision/CAS | pure/legacy immutable stores exist | complete residency identity, real crypto/key owner, restore semantics | `#111/#115/#134` |
| Materialization/UnitSet | materializer/unitizer v2 exist | typed producer chain, complete authoritative UnitSet, no invalid fallback | `#112/#113/#209` |
| Exact search | proof/denominator kernel exists | mature matchers + real owner inventory/readback | `#131` |
| Lexical | deterministic source exists | explicit Unicode profiles, exact source mappings and generation migration | `#122/#221` |
| Code intelligence | custom Rust enricher exists | replace production parser path with Tree-sitter + SCIP fact plane | `#223 → #224` |
| Qdrant process | real supervisor source exists | Windows containment, exact child identity, secret channel, qualification | `#120/#119` |
| Qdrant bridge | real 1.19 adapter exists | selected S9.5/S10.3/S11 migration and exact generation authority | `#200` |
| Publication | substantial pure state machine exists | one admission validator and owner-issued proof/readback | `#210` |
| Access/query | planner/executor/validator/projector source exists | actual live route, security checkpoints and source validation | `#117/#125/#127` |
| Handles/continuations | canonical kernels and legacy serving catalogs coexist | cut serving to range-bound live-authorized records | `#118` |
| Ranking/orientation | primitives exist; current fusion unsafe | closed exact arithmetic/profile and deterministic orientation | `#221 → #232 → #221` |
| Retention/purge | substantial pure kernels exist | owner-issued inventories/roots/typed receipts and durable journal | `#134 → #135` |
| Backup/restore/migration | vocabulary and partial coordinator exist | strict backup, same-owner restore, per-object re-encryption and cutover | `#243 → #244 → #245` |
| Documents | workers/contracts exist but disabled | isolated provider runtime and one qualified profile at a time | `#226 → #227/#216` |
| External research | no canonical product connector plane yet | checkpoint core and pinned provider releases | `#228 → #229/#230/#231` |
| Evaluation/release | pure model and plans exist | real runner, raw evidence, independent verifier, final package | `#233 → #234 → #240 → #215/#140` |

---

## 4. Highest-priority technical defects

### Critical authority defects

- Persistent quarantine can be bypassed by one-shot command paths.
- Read-only commands can initialize durable state.
- Safe-reader production composition fabricates admitted-root/security authority.
- Admission reads full denied bodies before policy evaluation.
- Publication restore accepts mutually consistent caller-created false digests.
- Publication stage/close/control/snapshot proofs are echoable ordinary structs.
- Portfolio updates overwrite historical immutable revisions.
- Purge/retention/restore use caller booleans/counts/receipts as proof in several paths.
- Actual handles can widen from one match to an arbitrary range within the source revision.

### Critical correctness/integration defects

- Current point identity, projection planner and selected Qdrant bridge use incompatible generations.
- Qdrant epoch conversion can accept `i64::MAX` through saturating round-trip.
- Global IDF remains representable in legacy production API.
- Qdrant operation deadline resets across RPC phases.
- Source identity planning is `O(batch × corpus)` and carries a hidden 100,000-source limit.
- Ordinary open/health/search can trigger hidden corpus migration/full verification.
- Directory and GC traversal allocate/collect before limits fire.
- Current fusion depends on floating accumulation/order, duplicate leg IDs and implicit weights.

### Critical lifecycle/resource defects

- CLI error paths can leak/unreap child processes.
- Config/status rereads mutable process/file state instead of active accepted snapshot.
- Query/open paths can perform hidden durable initialization/migration.
- Process-local replay or pin state is treated as if crash durable in several areas.
- One-shot mutations can leave root ownership `ACTIVE`.
- Multiple cleanup/GC/restore paths use path-based or incomplete object authority.

### New overlay findings F45–F55 — 2026-10-09

A fresh static pass over `crates/search-query/search-overlay/src/lib.rs` found eleven additional defects. Detailed remediation is recorded on PR `#130`.

- **F45 — HIGH:** declared TTL is not tied to actual `created_at/expires_at` lifetime.
- **F46 — CRITICAL:** one snapshot can mix access/purge/workspace revisions while advertising only the first entry's revisions.
- **F47 — HIGH:** snapshot digest omits semantic fields including `position_encoding` and per-entry security/view revisions.
- **F48 — HIGH:** saved references bypass `request.max_candidates`.
- **F49 — HIGH:** scan budget exhaustion aborts the whole query instead of returning the documented partial gap.
- **F50 — HIGH:** live-limit reduction does not enforce all accepted dimensions.
- **F51 — HIGH:** more expired entries than one lifecycle batch can permanently block snapshot creation.
- **F52 — HIGH:** recovery does not reconstruct the operation/idempotency ledger.
- **F53 — CRITICAL:** recovery mutates live state incrementally and can leave partial state on error.
- **F54 — HIGH:** an empty snapshot fabricates zero authority/currentness identities.
- **F55 — CRITICAL:** retrieve/merge do not reauthorize after snapshot creation.

Root direction:

```text
owner-derived digest/time/guard/checkpoint
→ coherent single-view snapshot
→ canonical digest over every semantic field
→ one shared candidate/step/byte budget
→ partial outcomes instead of discarded work
→ validate complete recovery batch
→ atomic state replacement
→ live reauthorization before retrieve/merge/save/output
```

---

## 5. Where the Frankenstein came from

The codebase did not grow only because too many files were generated. The same causal responsibility was implemented several times under different generations:

- legacy DIRECT catalog versus redb/source-registry authority;
- daemon-local root catalog versus typed RootRecord;
- legacy and canonical point identity;
- several provider-client/transport seams;
- daemon-local handles/continuations versus canonical packages;
- multiple config/argument parsers;
- process-local publication/replay proofs versus durable owner journal;
- watcher/sync booleans versus coherent SourceView currentness;
- custom parsers/matchers/Unicode/Git/document logic versus mature donors;
- packet-only PRs versus code landed directly on main.

The correct simplification is authority cutover followed by deletion, not mechanical file splitting.

```text
select one owner
→ bind real product caller
→ preserve focused fixtures
→ run minimum check/Clippy
→ delete superseded production path
→ only then split a still-large cohesive owner
```

---

## 6. Donor adoption summary

The full donor registry is maintained as a separate audit artifact; this repository file records only accepted donor decisions and execution ownership.

### Direct small dependencies

```text
lexopt
serde / serde_json
memchr
aho-corasick
regex-automata
unicode-ident
unicode-normalization
unicode-segmentation
pulldown-cmark
quick-xml
cargo_metadata
syn
Tree-sitter runtime + pinned grammar/query bundle
```

### Protocols/interchange

```text
SCIP
SARIF 2.1.0
in-toto Statement v1 / DSSE
W3C Web Annotation / IIIF
LSP 3.18
MCP 2026-07-28 as a leaf only
```

### Isolated providers

```text
Universal Ctags
rust-analyzer/scip-* indexers
Semgrep / CodeQL / Joern
GROBID / Docling / Tika / LiteParse/PDFium
OCRmyPDF / Tesseract and later OCR profiles
```

### Mature invariants only

```text
Sourcegraph/Zoekt dual indexed/exact routes
Glean/Kythe/Searchfox fact/anchor model
Lucene/Tantivy immutable generation commit
Nix GC root/reachability model
Vespa staged ranking
Aider Repo Map orientation
Zotero/Onyx connector checkpoint semantics
Anserini/Pyserini/BEIR run artifacts
PaperQA2/OpenScholar/STORM research workflow above Search
```

Never import these full stacks as a second product authority.

---

## 7. Open PR disposition

### Keep as execution authority

```text
#97   master coordinator
#200  selected Qdrant bridge base
#207  selected point identity base
#209  selected projection planner base
#210  selected publication implementation
#222  documentation/status cutover
```

### Current implementation/program specifications, branch may still be historical/non-mergeable

```text
#109 #110 #115 #116 #122 #128 #129 #130 #131 #132
#134 #136 #137 #138 #140
```

The body is current authority; an agent must verify whether the branch contains code or only an old packet before attempting merge.

### Tracking/gate PRs that must not be merged as code

```text
#99–#105
#106–#108 where main already contains direct-landed increments
#111–#114
#117–#121
#123–#127
#133 #135 #139
#184 #185 #190 #191 #193
```

These may remain open only as current acceptance checklists; they must not be presented as implementation branches.

Every open PR must have exactly one disposition:

```text
SELECTED_MERGE_BASE
CURRENT_PROGRAM_SPEC
TRACKING_ACCEPTANCE_ONLY
GATE_ONLY
DOCS_ONLY
SUPERSEDED_CLOSED
```

---

## 8. Exact next implementation queue

### Phase 1 — delete future custom code and establish shared foundations

```text
#237 canonical encoding/digests
#235 canonical provider client
#138 cargo_metadata + syn
#238 standard TOML config profile
#131 mature exact matchers
#122 explicit Unicode profiles
#241 compiled admission profiles
#226 Markdown/JATS
```

### Phase 2 — close source authority

```text
#100/#104/#105 native root/owner authority
#106→#108 authoritative redb cutover
#110 pre-admission/staging/identity index
#239 watcher hints/gaps
#128 coherent reconciliation
#218 immutable scope/corpus/portfolio authority
```

### Phase 3 — integrate selected indexed spine

```text
#207
→ #209
→ #210
→ #200
→ #117/#118/#125/#127
```

### Phase 4 — public usefulness

```text
#223→#224 code facts
#213 recipe schemas and served operations
#221-A→#232→#221-B ranking/orientation
#228→#229/#230/#231 scholarly corpora
```

### Phase 5 — lifecycle and qualification

```text
#134→#135
#243→#244→#245
#227 + selected document providers
#233→#234→#240→#137
#242 package
#215 qualification
#140 release
#130/#219 optional leaves last
```

---

## 9. Instructions for coding agents

```text
1. Read root/package AGENTS.md, #97 and exact assigned task.
2. Record base SHA, manager, worktree, exact owned files and forbidden neighbours.
3. Read exact donor version/spec/source named by the task.
4. Recheck checksum, license, MSRV, advisories and enabled feature closure.
5. Implement only the first coherent numbered phase.
6. Delete the replaced production implementation in the same series.
7. Run exact Rust 1.98 locked package check.
8. Run strict Clippy for affected packages.
9. Run only focused causal fixtures with nonzero case count.
10. Report result SHA, files, donor/features, outcomes and next blocker.
```

Stop instead of improvising when implementation would create a second authority/catalog/client/index/journal, expose a donor type publicly, require runtime download/network fallback, accept caller-created proof, hide normalization/transcoding/skipping, allocate before ceilings, replay blindly after unknown effect or retain two production implementations after cutover.

---

## 10. Evidence boundary

Historical evidence exists for selected older revisions, including a Windows Rust 1.98 daemon check and selected package Clippy results. It does not prove current-main product qualification.

Missing for current main:

- exact-head workspace strict Clippy;
- accepted Windows native owner/pipe/process/secret qualification;
- complete selected Qdrant schema/IDF/publication/query qualification;
- installed end-to-end product run;
- preregistered scale/resource/no-write/disclosure matrix;
- accepted release package.

No documentation/task update creates such evidence.

---

## 11. Final assessment

The project is recoverable without rewriting it from scratch. The source contains valuable and often sophisticated kernels. The main risk is continuing to add parallel authority and compatibility layers.

The shortest path is:

```text
freeze selected owners
→ adopt donors for low-level machinery
→ close source/root/control authority
→ integrate #207→#209→#210→#200
→ cut real provider/query/handle paths to it
→ delete legacy production paths
→ qualify exact installed product
```

Do not optimize for closing the maximum number of issues. Optimize for deleting duplicate semantics and making one product path impossible to misunderstand.
