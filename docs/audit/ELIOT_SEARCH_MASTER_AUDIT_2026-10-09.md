# ELIOT Search — единый актуальный аудит проекта

**Дата актуализации:** 2026-10-09  
**Репозиторий:** `UnknownAlienHuman/eliot-search`  
**Аудированный `main`:** `2ae7ecbdf7dced3cc951e5e162dbba10def03dbc`  
**Координатор:** #97  
**Product/release ready:** no  
**Controlled implementation wave ready:** yes

> Этот файл — основной repository entrypoint. Машинный owner/status graph находится в `docs/product/PACKAGE_STATUS.toml`; launch order — в `AGENT_LAUNCH_GATE_2026-10-09.md`; indexed donor/port map — в `INDEXED_SPINE_PORT_MAP_2026-10-09.md`.

---

## 1. Executive conclusion

ELIOT Search уже не scaffold. В репозитории существуют крупные реализации source admission/identity/registry, safe-reader, revisions/CAS, materialization/unitization, DIRECT search, access/query/handles, Qdrant supervisor/bridge, publication, retention/purge/restore и optional adapters.

Продукт не готов, потому что эти части ещё не сведены в одну каноническую вертикаль:

```text
one data-root/control authority
→ immutable root/source/SourceView/corpus scope
→ retained preparation + complete typed UnitSet
→ exact projection manifest
→ durable publication transaction
→ real Qdrant generation
→ identical authorized retrieval/IDF population
→ exact candidate/source readback
→ live reauthorization
→ range-bound output
```

Корневая проблема — параллельная семантика:

- legacy DIRECT catalog против redb/source-registry authority;
- several owner/open/quarantine paths;
- parent-directory root hints and fabricated security state;
- several identity/digest/canonical writers;
- multiple provider-client/transport paths;
- legacy and proposed point identity/planner/schema generations;
- process-local replay/proof state instead of durable owner journal;
- watcher/sync booleans instead of coherent immutable SourceView;
- mutable portfolio records that can overwrite historical meaning;
- custom low-level parsers/matchers/Unicode/Git/document code instead of mature donors;
- packet/tracking/code PRs without a single disposition.

Correct simplification:

```text
select one authority owner
→ port only useful donor logic
→ bind the real product caller
→ delete superseded production path
→ run locked check + strict Clippy
→ integrate one end-to-end path
→ qualify exact installed generation
```

---

## 2. GitHub reconciliation already applied

### Historical issue wave

- #48 closed as completed after normative registry readback.
- #51–#89 closed as `not_planned`: superseded, not implemented or qualified.
- Old P00/P01/P02/W0/W1/W2 issues are no longer implementation authority.

### Documentation/status cutover

Merged #222, #255 and #265 delivered:

- schema-v3 `PACKAGE_STATUS.toml` for every active workspace member;
- correction of 26 false active README scaffold claims;
- current master audit and launch gate;
- explicit source-donor versus executable-task graph;
- indexed-spine port map and findings F56–F61.

`SOURCE_PRESENT` never means `PRODUCT_PATH_INTEGRATED`, `LIVE_NATIVE_QUALIFIED` or `PUBLIC_CAPABILITY_ENABLED`.

### Closed packet/source-donor PRs

Low-level packet PRs:

```text
#122 → #252–#254
#131 → #246–#249
#138 → #250–#251
```

Indexed source donors:

```text
#207 f0ac8a1 → #256
#209 e5c14cd → #257/#258/#259
#210 a697e17 → #260/#261
#200 615d64f → #258/#262/#263/#264
```

Root/source/control tracking packets:

```text
#100 → #267/#215
#104 → #267
#105 → #266
#106 → #268/#269/#261
#107 → #268
#108 → #269
#110 → #241/#267/#269/#270/#271
#128 → #239/#267/#269/#270/#271/#272
```

Closed branches remain read-only source/fixture/history archives. They are not merge bases.

---

## 3. Controlled agent launch

### Wave 1 — run now

```text
coding manager:        #237 canonical codec/digest foundation
research/docs manager: #253 exact Unicode full-case-fold decision
```

### After #237 — disjoint foundations

```text
indexed:
  #256 S11 point identity
  #257 authoritative UnitSet v3
  #258 shared S9.5/S10.3 contract

root/control:
  #266 typed data-root open/owner authority

code reduction:
  #235 provider client
  #238 TOML/config
  #241 GlobSet admission
  #246 literal matcher
  #250 cargo_metadata tooling
  #252 code identifiers
  #226 Markdown/JATS
```

One root dependency/Cargo.lock integration manager merges at a time.

---

## 4. Root/source/control graph

```text
#237 → #266
#266 + #241 → #267

#237 + #266
→ #268 complete legacy inventory/staged redb mapping
→ #269 atomic cutover to one redb authority

#237 + #241 + #267 + #269
→ #270 pre-admission before bytes
       + one source-view identity index
       + bounded immutable staging
       + exact redb source commit

#239 + #267 + #269 + #270
→ #271 durable root/workspace reconcile
       + immutable coherent SourceView

#269 + #271
→ #272 immutable corpus/portfolio revisions
       + guarded aliases
       + exact authorized scope compiler
```

This graph removes create-on-read, one-shot quarantine bypass, parent-as-root, constant security barrier, dual control authority, read-first ingestion and mutable scope history.

---

## 5. Canonical indexed spine

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

#272 + all root/source/access/handle/supervisor owners
→ #264 fresh-generation product qualification
```

No old collection generation is adopted. Identity, payload, manifest and schema changes require a fresh `CollectionGenerationId` and rebuild from retained source/preparation.

---

## 6. Current subsystem state

| Subsystem | Current source | Main unresolved boundary | Owner |
|---|---|---|---|
| Canonical values/digests | substantial codec plus duplicated/fake digest implementations | one real algorithm owner and guard | #237 |
| CLI/provider client | typed session code inside CLI plus legacy shim | one reusable client and one native endpoint | #235 → #116 |
| Config | typed semantic model plus handwritten TOML/SHA | maintained parser, canonical digest, real apply/readback | #238 → #109 |
| Data-root/runtime | OS/durable owner kernels exist | typed inspect/mutate/init/recovery modes and uniform quarantine | #266 |
| Safe read | final-handle/race mechanics exist | active RootRecord, live barrier/access/purge, native location | #267 |
| Control migration | bounded source/object pages and staged redb scaffolding exist | complete mapping/accounting and verified staged candidate | #268 |
| Control authority | redb source exists; legacy still serves paths | atomic current-authority cutover, no dual write/fallback | #269 |
| Source admission | large policy kernel exists | honest compiled path policy and pre-admission before bytes | #241 → #270 |
| Source identity/ingestion | substantial identity/registry/DIRECT code exists | one identity index, bounded staging, redb commit | #270 |
| Source currentness | reconcile/currentness kernels exist | watcher gaps, durable root state machine, all-root revalidation | #271 |
| Scope/corpora | pure registry shapes exist | immutable history, guarded aliases, one population compiler | #272 |
| Revision/CAS | pure and legacy stores exist | canonical residency/key owner and lifecycle integration | #111/#115/#134 |
| Unitization | v2 spans/digests exist | contract UnitId/UnitKind/NativeAnchor and complete UnitSet | #257 |
| Exact search | proof plane and custom KMP exist | mature engines, #272 denominator and handles | #246→#249 |
| Lexical | deterministic analyzer exists | XID/normalization/full-fold profiles and generation migration | #252→#254 |
| Code intelligence | custom Rust enricher exists | Tree-sitter baseline plus SCIP precise facts | #223→#224 |
| Qdrant process | supervisor source exists | Windows containment, child/secret/socket qualification | #120/#264 |
| Point identity | legacy FNV/registry profile exists | S11 through #237 | #256 |
| Indexed contract | copied/drifting schema/filter definitions exist | one S9.5/S10.3/epoch contract | #258 |
| Projection planner | legacy main plus donor snapshot | verified UnitSet + shared schema + exact identity | #259 |
| Publication | substantial pure machine exists | one admission path, owner-issued proofs, durable recovery | #260→#261 |
| Qdrant bridge | real adapter plus donor snapshot | shared identity/schema, route admission, exact effects/readback | #262→#263 |
| Access/query | planner/executor/validator/projector source exists | #272 scope, live route and output reauthorization | #117/#125/#127/#264 |
| Handles | canonical kernels and legacy catalogs coexist | range-bound current-authority serving | #118/#264 |
| Ranking/orientation | primitives exist; unsafe floating/default fusion | exact arithmetic/profile and deterministic orientation | #221→#232→#221 |
| Retention/purge | substantial kernels exist | owner-issued roots/inventory/readback and journal | #134→#135 |
| Backup/restore | models and partial composition exist | strict backup, same-owner restore, per-object migration | #243→#245 |
| Documents | contracts/workers exist but disabled | independent sandboxed qualified profiles | #226→#227/#216 |
| Research connectors | no canonical product connector plane | checkpoint core and pinned releases through #272 | #228→#231 |
| Evaluation/release | pure models/plans exist | real runner, verifier, package and installed evidence | #233→#240→#215/#140 |

---

## 7. Findings F01–F68 — priority summary

The downloadable detailed audit retains the full F01–F68 source analysis. Load-bearing classes follow.

### Authority failures

- quarantine can be bypassed by one-shot paths;
- read-only operations can initialize durable state;
- safe-reader root/security/location authority is fabricated in composition;
- denied files can be read before policy;
- caller-created digests/receipts/booleans act as proof in publication/lifecycle paths;
- portfolio revisions can overwrite historical meaning;
- handles can widen disclosure;
- overlay snapshots can mix/omit security-currentness fields;
- recovery can partially mutate before full validation.

### Correctness/integration failures

- identity/planner/bridge generations are incompatible;
- Qdrant epoch float round-trip can accept `i64::MAX`;
- global/wider IDF remains representable in legacy code;
- timeout resets across RPC phases;
- source identity planning has `O(batch × corpus)` behavior and a hidden 100k limit;
- open/health/query can trigger hidden initialization/migration/full verification;
- directory/GC traversal can allocate before ceilings;
- rank fusion depends on `f64`, arrival order, duplicate legs and implicit weights;
- exact/index/query paths use inconsistent currentness/security semantics.

### Lifecycle/resource failures

- CLI child processes can survive failure paths;
- status can reread mutable config rather than applied state;
- process-local replay/pins are confused with durability;
- one-shot mutations can leave root ownership active;
- GC/restore use incomplete/path-based authority;
- overlay expiry/recovery can block progress or lose idempotency.

### F56–F61 — indexed donor/authority defects

- stacked donor branches are not merge bases;
- UnitManifest v2 cannot prove complete typed UnitSet;
- #207 duplicates canonical ownership;
- #209 accepts caller/self-asserted completeness;
- #200 duplicates identity/schema/filter, has epoch and local-ledger defects;
- #210 is a small integrity delta, not complete publication authority.

### F62 — create-on-read and quarantine bypass

`DirectStore::open` creates control/revision directories, namespace and source log. Read-only commands using it can initialize state. Service quarantine and one-shot paths do not share one root-admission capability. #266 separates inspect, mutate, initialize and named recovery.

### F63 — parent directory is treated as admitted root

`read_file_snapshot` passes `absolute.parent()` into the safe-reader adapter. Containment is proved only against an arbitrary immediate parent, not an active durable RootRecord. #267 replaces this with an owner-issued root-read binding.

### F64 — fabricated live security and location

The safe-reader adapter uses constant barrier revision `1`, always returns `ReadSecurityDisposition::Permitted` and can classify sources `LocalFixed` without native volume/share proof. #267 requires live owner/security/access/purge revalidation and qualified native location.

### F65 — control migration/cutover remains dual-authority

Legacy DIRECT and redb source both exist. Migration pages are useful but incomplete; ordinary serving still reaches legacy paths. #268 builds one fully accounted staged candidate; #269 atomically switches all normal paths and forbids fallback/dual write.

### F66 — ingestion reads first and scales as a cross product

Current composition can read full bodies before admission, clone/scan existing identities per file and retain too much batch state. #270 enforces pre-admission, one verified identity index, bounded staging/backpressure and redb commit/readback.

### F67 — watcher/per-root success cannot prove SourceView currentness

Watcher silence is not truth; sequential root reconciliation without final all-root reobservation can publish a stale workspace. #271 persists root/workspace state and publishes only immutable exact complete/partial SourceView revisions after final revalidation.

### F68 — mutable corpus/portfolio storage breaks historical scopes

Keying current records only by portfolio/corpus ID lets later publication change the meaning of old references. #272 stores append-only `(id, revision)` records and separate guarded aliases, then compiles one exact population shared by exact/retrieval/IDF/validation/ranking.

---

## 8. Donor adoption policy

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

## 9. Exact implementation sequence

### Now

```text
#237
#253
```

### After #237 — disjoint foundations

```text
indexed:     #256 #257 #258
root/control:#266
other:       #235 #238 #241 #246 #250 #252 #226
```

### Root/source/control serialization

```text
#266 + #241 → #267
#237 + #266 → #268 → #269
#237 + #241 + #267 + #269 → #270
#239 + #267 + #269 + #270 → #271
#269 + #271 → #272
```

### Indexed serialization

```text
#256 + #257 + #258 → #259
#259 → #260 → #261
#256 + #258 → #262
#261 + #262 → #263
#272 + all required owners → #264
```

### Other chains

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

## 10. Agent rules

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
- donor types across product-domain boundaries;
- runtime downloads or hidden network fallback;
- caller-issued proof;
- path/parent/PID/quiet watcher as authority;
- hidden normalization/transcoding/skipping;
- allocation before ceilings;
- blind replay after possible external effect;
- a closed donor/packet branch as base;
- uncoordinated root dependency/Cargo.lock edits.

---

## 11. Evidence boundary and final assessment

Historical/package evidence exists, but current integrated product evidence does not.

Still unproven on current main:

- exact-head workspace strict Clippy;
- accepted Windows root/pipe/process/secret boundary;
- atomic redb authority cutover;
- coherent immutable SourceView/corpus scope;
- selected S9.5/S10.3/S11 collection generation;
- durable publication/restart recovery against real Qdrant;
- identical authorized retrieval/IDF population;
- installed end-to-end product;
- preregistered scale/resource/no-write/disclosure matrix;
- accepted Windows release package.

The project is recoverable without a rewrite. The shortest path is:

```text
#237 common authority
→ #266/#267/#268/#269/#270/#271/#272 source authority
→ #256/#257/#258 indexed foundations
→ #259/#260/#261/#262/#263
→ #264 fresh-generation product proof
→ delete remaining legacy product paths
→ installed qualification and release
```

Do not launch a broad swarm. Expand only after each shared authority owner merges and publishes a new accepted base SHA.
