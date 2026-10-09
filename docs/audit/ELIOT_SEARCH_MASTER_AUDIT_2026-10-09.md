# ELIOT Search — единый актуальный аудит проекта

**Дата актуализации:** 2026-10-09  
**Репозиторий:** `UnknownAlienHuman/eliot-search`  
**Аудированный `main`:** `f1757d5b528d63abde28eff443f1070ff88fff4d`  
**Координатор:** #97  
**Product/release ready:** no  
**Controlled implementation wave ready:** yes

> Этот файл — основной repository entrypoint. Машинный owner/status graph находится в `docs/product/PACKAGE_STATUS.toml`; точный launch order — в `AGENT_LAUNCH_GATE_2026-10-09.md`; indexed donor/port map — в `INDEXED_SPINE_PORT_MAP_2026-10-09.md`.

---

## 1. Executive conclusion

ELIOT Search уже не scaffold. В репозитории существуют крупные реализации source admission/identity/registry, safe-reader, revisions/CAS, materialization/unitization, DIRECT search, access/query/handles, Qdrant supervisor/bridge, publication, retention/purge/restore и optional adapters.

Продукт не готов, потому что эти части ещё не сведены в одну каноническую вертикаль:

```text
authoritative root/source/revision
→ retained preparation
→ complete typed UnitSet
→ exact projection manifest
→ durable publication transaction
→ real Qdrant generation
→ identical authorized retrieval/IDF population
→ exact candidate/source readback
→ live reauthorization
→ range-bound output
```

Корневая проблема проекта — не только объём кода. Одна ответственность многократно реализована в разных поколениях:

- legacy DIRECT catalog против authoritative source/control owners;
- несколько identity/digest/canonical writers;
- несколько provider-client/transport paths;
- legacy и proposed point identity/planner/schema generations;
- process-local replay/proof state вместо durable owner journal;
- watcher/sync booleans вместо coherent SourceView;
- custom low-level parsers/matchers/Unicode/Git/document code вместо mature donors;
- packet PRs, tracking PRs и реальные code branches без ясной disposition.

Правильная стратегия:

```text
select one owner
→ port only useful donor logic
→ bind actual caller and authority
→ delete superseded production path
→ run minimum locked check/strict Clippy
→ integrate one product path
→ qualify exact installed generation
```

---

## 2. GitHub reconciliation already applied

### Historical issue wave

- #48 closed as completed after normative registry readback.
- #51–#89 closed as `not_planned`: superseded, not implemented or qualified.
- Old P00/P01/P02/W0/W1/W2 issues are no longer implementation authority.

### Documentation/status cutover

Merged #222 and #255 delivered:

- one schema-v3 `PACKAGE_STATUS.toml` for all active workspace members;
- 26 active README corrections that previously claimed substantive packages were unimplemented;
- one current master audit and agent launch gate;
- packet-only PR retirement and executable successor issues.

`SOURCE_PRESENT` never means `PRODUCT_PATH_INTEGRATED`, `LIVE_NATIVE_QUALIFIED` or `PUBLIC_CAPABILITY_ENABLED`.

### Closed packet/source-donor PRs

Packet-only September PRs:

```text
#122 → #252–#254
#131 → #246–#249
#138 → #250–#251
```

Indexed source-donor snapshots:

```text
#207 head f0ac8a1 → #256
#209 head e5c14cd → #257/#258/#259
#210 head a697e17 → #260/#261
#200 head 615d64f → #258/#262/#263/#264
```

All are closed and preserved only for source/fixture archaeology. They are not merge bases.

---

## 3. Controlled agent launch

### Wave 1 — run now

```text
coding manager:       #237 canonical codec/digest foundation
research/docs manager #253 exact Unicode full-case-fold decision
```

Only these two may start from current main without shared-owner conflict.

### Wave 2A — indexed foundations after #237

```text
#256 exact S11 point identity
#257 authoritative typed UnitSet / manifest v3
#258 shared S9.5/S10.3 payload/schema/eligibility/epoch contract
```

These three own disjoint package surfaces after #237.

### Wave 2B — donor code reduction after #237

```text
#235 canonical provider client
#238 maintained TOML/config cutover
#241 bounded GlobSet admission policy
#246 mature single-literal matcher
#250 cargo_metadata tooling
#252 code_identifiers@1
#226 Markdown/JATS profiles
```

Only one root dependency/Cargo.lock integration manager merges at a time.

---

## 4. Canonical indexed spine

```text
#237
├─ #256 point identity
├─ #257 UnitSet v3
└─ #258 shared indexed contract

#256 + #257 + #258
→ #259 projection planner
→ #260 prepared-generation admission
→ #261 durable publication effects/recovery

#256 + #258
→ #262 Qdrant schema/route/codec boundary

#261 + #262
→ #263 live Qdrant mutations/readback/scoped query

all above + root/source/access/handle/supervisor owners
→ #264 fresh-generation product qualification
```

No old collection generation is adopted. Identity, payload, manifest and schema changes require a fresh `CollectionGenerationId` and rebuild from retained source/preparation.

---

## 5. Current subsystem state

| Subsystem | Current source | Main unresolved boundary | Owner |
|---|---|---|---|
| Canonical values/digests | substantial canonical codec plus duplicated/fake digest implementations | one real algorithm owner and repository guard | #237 |
| CLI/provider client | typed client/session code exists inside CLI plus legacy shim | one reusable client and one native endpoint | #235 → #116 |
| Config | typed merge/descriptor model plus handwritten TOML/SHA | maintained parser, canonical digest, real apply/readback | #238 → #109 |
| Root/runtime | guards and owners exist | one retained authority across all stores/processes | #100/#104/#105 |
| Source admission | large policy kernel exists | pre-admit before bytes; honest path policy; native location/root proof | #241 → #110 |
| Source currentness | registry/reconcile source exists | watcher gaps and coherent all-root SourceView | #239 → #128 → #218 |
| Revision/CAS | pure and legacy stores exist | canonical residency/key owner and lifecycle integration | #111/#115/#134 |
| Unitization | v2 deterministic spans/digests exist | typed contract UnitId/UnitKind/NativeAnchor and complete UnitSet authority | #257 |
| Exact search | proof plane and custom KMP exist | mature engines, authoritative denominator and handles | #246→#249 |
| Lexical | deterministic analyzer exists | explicit XID/normalization/case-fold profiles and generation migration | #252→#254 |
| Code intelligence | custom Rust enricher exists | Tree-sitter baseline plus SCIP precise fact plane | #223→#224 |
| Qdrant process | supervisor source exists | Windows containment, child/secret/socket identity and live qualification | #120/#264 |
| Point identity | legacy FNV/registry profile exists | exact S11 port through #237 | #256 |
| Indexed contract | copied/drifting payload/filter/schema definitions exist | one provider-neutral S9.5/S10.3 contract | #258 |
| Projection planner | legacy main plus closed donor snapshot | verified UnitSet + shared schema + exact identity | #259 |
| Publication | substantial pure machine exists | one admission path, owner-issued proofs, durable recovery | #260→#261 |
| Qdrant bridge | real current adapter plus closed donor snapshot | shared identity/schema, route admission, exact effects/readback | #262→#263 |
| Access/query | planner/executor/validator/projector source exists | actual live route and reauthorization at every output boundary | #117/#125/#127/#264 |
| Handles/continuations | canonical kernels and legacy catalogs coexist | range-bound current-authority serving | #118/#264 |
| Ranking/orientation | primitives exist; unsafe floating/default fusion remains | closed exact arithmetic/profile and deterministic orientation | #221→#232→#221 |
| Retention/purge | substantial pure kernels exist | owner-issued root/inventory/readback and durable journal | #134→#135 |
| Backup/restore | models and partial composition exist | strict backup, same-owner restore, per-object migration | #243→#245 |
| Documents | contracts/workers exist but disabled | independent sandboxed qualified profiles | #226→#227/#216 |
| Research connectors | no canonical product connector plane | checkpoint/event core and pinned releases | #228→#231 |
| Evaluation/release | pure models/plans exist | real runner, verifier, package and installed evidence | #233→#240→#215/#140 |

---

## 6. Findings F01–F61 — priority summary

The complete detailed audit artifact carries F01–F61. The load-bearing classes are summarized here.

### Authority failures

- quarantine can be bypassed by one-shot command paths;
- read-only operations may create durable state;
- root/security authority is fabricated in some safe-reader composition;
- denied files can be fully read before path policy;
- caller-created digest/receipt/boolean values are accepted as proof in several publication/lifecycle paths;
- historical portfolio revisions can be overwritten;
- range handles can widen disclosure;
- overlay snapshots can mix security/currentness revisions and omit them from digest;
- recovery paths can mutate partially before validation completes.

### Correctness/integration failures

- current identity/planner/bridge generations are incompatible;
- Qdrant epoch float round-trip can accept `i64::MAX`;
- global/wider IDF remains representable in legacy code;
- per-RPC timeout resets violate one-operation deadline;
- source identity planning has `O(batch × corpus)` behavior and a hidden 100k limit;
- open/health/query can trigger hidden migration/full verification;
- directory/GC traversal can allocate/collect before ceilings;
- current rank fusion depends on `f64`, arrival order, duplicate legs and implicit weights;
- exact/open/query paths use different currentness/security semantics.

### Lifecycle/resource failures

- child processes can survive CLI failure paths;
- health/status may reread mutable config rather than applied state;
- process-local replay/pin maps are confused with durability;
- one-shot mutations can leave root ownership active;
- GC/restore paths still rely on incomplete/path-based authority;
- overlay expiry/recovery budgets can deadlock progress or lose idempotency state.

### F56 — stacked indexed donor branches are not merge bases

#207/#209/#210/#200 were created on obsolete stacked bases and duplicate current ownership. Port exact useful logic only.

### F57 — UnitManifest v2 cannot prove the projection denominator

It has spans/ordinals/digests but not contract UnitId, UnitKind, NativeAnchor, structural identity or configuration predicate. #257 must produce a verified typed UnitSet v3.

### F58 — #207 duplicates canonical ownership

Its S11 shape and collision fixtures are useful; manual CBOR/direct BLAKE3 are rejected in favor of #237.

### F59 — #209 accepts self-asserted completeness

A caller `Vec<PreparedUnit>` may be a self-consistent subset. #259 consumes #257 `VerifiedUnitSet`, never a caller list/count/digest.

### F60 — #200 contains several authority defects

It duplicates point/schema/filter logic, has the float epoch defect, weak physical-name route authority, a process-local replay ledger and ambiguous vector support. #262/#263 port only useful Qdrant translation/live code.

### F61 — #210 is not a complete publication implementation

It is a small integrity delta. #260 creates one submit/restore admission path; #261 creates durable owner-issued effect proofs and recovery.

---

## 7. Donor adoption policy

### Direct small libraries

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

### Protocol/interchange

```text
SCIP
SARIF 2.1.0
in-toto Statement / DSSE
W3C Web Annotation / IIIF
LSP 3.18
MCP as a dated thin leaf only
```

### Isolated providers

```text
Universal Ctags
rust-analyzer/scip-* indexers
Semgrep / CodeQL / Joern
GROBID / Docling / Tika / PDF/OCR providers
```

### Invariants/benchmarks only

```text
Sourcegraph/Zoekt dual indexed/exact route
Glean/Kythe/Searchfox fact/anchor model
Lucene/Tantivy immutable commit model
Nix reachability/GC roots
Vespa staged ranking
Aider Repo Map orientation
Zotero/Onyx checkpoint semantics
Anserini/Pyserini/BEIR run artifacts
PaperQA2/OpenScholar/STORM workflows above Search
```

No donor may introduce a second source, access, control, index, publication or result authority.

---

## 8. Exact implementation sequence

### Now

```text
#237
#253
```

### After #237 — disjoint foundations

```text
indexed: #256 #257 #258
other:   #235 #238 #241 #246 #250 #252 #226
```

### Indexed serialization

```text
#256 + #257 + #258 → #259
#259 → #260 → #261
#256 + #258 → #262
#261 + #262 → #263
all required owners → #264
```

### Other serialized chains

```text
#246→#247→#248→#249
#250→#251
#252+#253→#254
#235→#116→#130/#219
#223→#224→#225→#236
#228→#229→#230→#231
```

### Later lifecycle and qualification

```text
#134→#135
#243→#244→#245
#233→#234→#240→#137
#242 package
#215 installed qualification
#140 release
```

---

## 9. Agent rules

Every implementation agent:

1. starts from exact current accepted main in one fresh worktree;
2. owns only paths named by the issue;
3. reads exact donor source/version before coding;
4. records checksum/license/MSRV/advisories/features;
5. implements one coherent numbered phase;
6. deletes the replaced production path in the same series;
7. runs Rust 1.98 locked package check;
8. runs strict Clippy on affected packages;
9. runs only focused causal fixtures with nonzero count;
10. reports result SHA, changed files, donor choices, evidence and next blocker.

Stop instead of improvising if the change would create:

- a second authority/catalog/client/index/journal/codec;
- a donor type across product-domain boundaries;
- runtime downloads or hidden network fallback;
- caller-issued proof;
- hidden normalization/transcoding/skipping;
- allocation before accepted ceilings;
- blind replay after possible external effect;
- a closed donor branch as working base;
- an uncoordinated root dependency/Cargo.lock edit.

---

## 10. Evidence boundary

Evidence that exists is historical and package-specific. It does not establish current integrated product readiness.

Not yet proven at current main:

- exact-head workspace strict Clippy;
- accepted Windows root/pipe/process/secret boundary;
- selected S9.5/S10.3/S11 collection generation;
- durable publication/restart recovery against real Qdrant;
- identical authorized retrieval/IDF population;
- installed end-to-end product;
- preregistered scale/resource/no-write/disclosure matrix;
- accepted Windows release package.

Documentation and issue changes never create runtime qualification.

---

## 11. Final assessment

The project is recoverable without rewriting it from scratch. Its useful kernels should be retained, but parallel semantic ownership must be removed aggressively.

The shortest route is:

```text
#237 common authority
→ disjoint donor-based foundations
→ #256/#257/#258
→ #259/#260/#261/#262/#263
→ #264 fresh-generation product proof
→ delete remaining legacy product paths
→ full installed qualification and release
```

Do not launch a broad swarm. Run the controlled wave defined by the launch gate, then expand only after each shared owner merges and publishes a new accepted base SHA.
