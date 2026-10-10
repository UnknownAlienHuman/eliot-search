# ELIOT Search — complete implementation and release map

**Reviewed source:** `1a6d17d073c5f6d1fe50f21082439f1965d24ef8`. **Review date:** 2026-10-09, America/New_York.

This is the current execution map **for the whole project**, not only Wave 2. It supersedes scheduling in dated wave packets and old coordinator descriptions. Architecture Part I and accepted ADRs still define product semantics. Exact issues define implementation acceptance. This document is static development documentation, never a product input, task scheduler, lease system or source of runtime authority.

## 1. Start here and continue without another planning round

Read [root instructions](../../AGENTS.md), [architecture](../architecture/README.md), [ADR 0005](../adr/0005-standalone-search-product-and-controller-boundary.md), [ADR 0006](../adr/0006-agent-analysis-framework-product-scope.md), this map, the exact issue, nearest package AGENTS.md/FUNCTIONS.md and current callers.

The next normal source slice is [#256](https://github.com/UnknownAlienHuman/eliot-search/issues/256). Do not repeat delivered #237/#253/#258/#250. After every merged slice, continue to a ready node below from the new exact main. No new chat permission or specially named next-wave SHA is required. Record the actual base/result SHA in each PR; never reset another dirty worktree.

One manager owns one writer worktree, root dependency pins, Cargo.lock and merges. Five to ten subagents perform bounded read/research/review; only the manager integrates repository changes under current root policy. One source PR at a time; no parallel writer branches or stacked unmerged prerequisites. A dependency is an actual API/artifact requirement, not a number's position in a list.

[PACKAGE_STATUS.toml](PACKAGE_STATUS.toml) records package state and owners. It is not the dependency graph and cannot add dependencies by ordering independent owners. [Master audit](../audit/ELIOT_SEARCH_MASTER_AUDIT_2026-10-09.md) records findings/evidence. Closed packet branches remain source/history donors only.

## 2. Delivered versus remaining

| Delivered issue | Merged PR | Boundary actually delivered |
|---|---|---|
| #237 | [#319](https://github.com/UnknownAlienHuman/eliot-search/pull/319) | Bounded canonical streaming, real digests, explicit restore/compute and source guard. |
| #253 | [#322](https://github.com/UnknownAlienHuman/eliot-search/pull/322) | Unicode 18 C+F and separate T mapping decision, no normalization, 39 goldens; not a tokenizer. |
| #258 | [#323](https://github.com/UnknownAlienHuman/eliot-search/pull/323) | One 20-field/19-index schema, scoped population and generation-local epoch contract. |
| #250 | [#326](https://github.com/UnknownAlienHuman/eliot-search/pull/326) | Bounded Cargo metadata/status tooling; actual whole-workspace result remained distinct. |

#318 repaired pre-existing tooling. #328 repaired documentation. #324 still needs the unchanged real validator rerun; #325 covers one frozen legacy constant; #327 covers the affected Rustls patch. Reported source checks are the implementation agent's evidence, not new executions by this documentation review. No installed/product/release PASS follows from this table.

## 3. Corrections that apply project-wide

**Build-safe migrations.** New API → actual consumer cutover → deletion. An unchanged legacy export may temporarily serve only inventoried old callers with a named removal owner. It cannot enter a new generation, become fallback or gain new callers. #256 → #259/#262 → #329 is the point-identity case. Compile immediate reverse consumers before deleting APIs.

**Client/native split.** #235 can merge a reusable typed core while unchanged external connection assembly remains. #332 installs the real native endpoint and removes old TCP/token/line serving. Core completion is not native-client completion; neither phase waits circularly for the other.

**Readiness.** Configured, Operational and ReleaseQualified are separate. #333/#277 use actual current owner observations for operational state. The running product must not depend on #234/#240 or its own future final test report. Optional artifacts still require explicit accepted profiles; feature presence does not enable them.

**Proof construction.** Adapters return untrusted typed observations. The responsible owner validates expected inputs and independent readback and constructs its private proof. Do not ask another crate to instantiate an inaccessible constructor, and do not replace that boundary with a public success Boolean. Publication phases are #261.model → #261.control → #263.

**Digest ownership.** Legacy raw SHA-256 differs from domain-prefixed SHA-256. Preserve bytes through the existing shared owner or create an explicit new profile. #320 owns admission crypto before #241 matching; #316 owns materializer crypto before #331; #315 and #321 have different daemon scopes. Never blanket-reclassify canonical-ledger exceptions as accepted.

**Preparation.** #257 owns UnitSet v3. #226 transforms source formats and #331 persists verified bundles; neither invents a temporary UnitSet, catalogue or manifest authority. #330 owns protected immutable storage. No zero or `invalid` profile may become successful preparation.

**Qualification.** Formats/verifier implementation → runner implementation → fixed candidate → installed runs → independent review → release. Product packages do not depend on evaluation/attestation code. Package tooling can be implemented with synthetic payloads; final packaging waits for the declared real feature closure. Signed/tested bytes must be the bytes published.

## 4. Full dependency table

Each row is an existing issue or a named phase of it. A suffix is not a new GitHub issue. `requires` lists real prerequisites. `done` is dependency evidence, not work to repeat. `extension` must be implemented and qualified if selected for shipping; explicitly not shipped does not mean implemented. Proposed new package names are declarations, not claims they already exist.

Keys in `owners` resolve in section 5. Detailed issue bodies specify exact allowed files, cases and completion criteria. Owner paths are not permission for a whole-package rewrite. Source slices use minimal locked Rust 1.98 check plus strict Clippy for changed packages and immediate reverse consumers. Broad product tests remain late.

```text
key | kind | requires | owners | donor | exit
237 | done | - | contracts,xtask | canonical | delivered by319
253 | done | - | docs | unicode | decision/goldens delivered by322
258 | done | 237 | contracts | index | shared indexed contract delivered by323
250 | done | 237 | xtask | cargo | bounded metadata delivered by326
324 | maintenance | 250 | docs,xtask | cargo | rerun unchanged validator after README repair
325 | maintenance | 237,258 | contracts | canonical | resolve legacy bounds constant without relabelling
327 | security | 250 | bridge | index | minimal reviewed Rustls patch and affected closure checks
256 | core | 237,258 | point | index | stateless S11; old callers unchanged until329
257 | core | 237,258 | unitizer | canonical | verified complete typed UnitSet v3
266 | core | 237 | runtime,daemon | native | one inspect/mutate/init/recovery root capability
235 | core | 237 | client,cli | cli | one typed client core; native cutover332
315 | maintenance | 237 | daemon | canonical | shared SHA execution with legacy exact-byte parity
320 | maintenance | 237 | admission | canonical | admission crypto migration, no glob semantics
316 | maintenance | 237 | materializer | canonical | versioned materializer digest migration
238 | core | 237 | config,contracts | config | bounded TOML visitor and explicit fingerprint migration
241 | core | 320 | admission | glob | effective bounded relative-byte policy
246 | core | 237 | exact | match | mature literal engine preserving chunk/overlap/ASCII behavior
247 | core | 246 | exact | match | bounded multi-literal profile
248 | core | 247 | exact | match | bounded non-backtracking regex profile
252 | core | 237 | lexical | unicode | code XID+NFC profile, original ranges
254 | core | 253,252 | lexical | unicode | UAX29 prose plus accepted no-normalization full folding
251 | maintenance | 250 | xtask | cargo | Syn source-shape checks, compiler-required ambiguity
268 | core | 266 | control,daemon | control | complete legacy inventory and staged redb candidate
269 | core | 268 | control,daemon | control | atomic cutover to one control authority
267 | core | 266,269,241 | reader,registry,daemon | native | real RootRecord/final-handle/live-security read binding
307 | core | 237 | secrets | crypto | purpose-bound lifecycle and finite selected/candidate/retiring leases
308 | core | 307,266 | secrets_windows,secrets | crypto | real current-user Windows secret backend
309 | core | 308 | revision_crypto,revision | crypto | AEAD envelope, exact key reference and nonce lifetime
330 | core | 266,269,309 | revision,daemon | control | real residency-bound immutable publication/readback
194 | core | 269 | control,contracts,protocol | control | durable binding and coherent grant-policy snapshot
333 | core | 238,269,308 | config,control,daemon | config | durable applied config; noncircular operational readiness
332 | core | 235,266,194,308 | cli,daemon,client | native | one real native provider/CLI; retire legacy serving
334 | core | 266,308,327 | supervisor,daemon | native | exact owned authenticated Windows Qdrant process
270 | core | 267,269,241,330 | admission,identity,registry,daemon | control | pre-admission, one identity index, bounded staging/commit
239 | core | 266 | runtime,reconcile,daemon | watch | hints and loss gaps, never currentness authority
271 | core | 270,239 | reconcile,registry,daemon | watch | durable all-root reconcile and coherent immutable view
272 | core | 271,269 | registry,access,daemon | control | immutable corpus/portfolio history and scope compiler
335 | core | 267,269,330,270 | reader,registry,daemon | git | safe local Git ODB through patched donor plumbing
331 | core | 257,316,330,269 | materializer,unitizer,revision,daemon | docs | persisted verified representation/maps/UnitSet bundle
226.md | core | 257,316 | materializer,unitizer | docs | inert Markdown profile; no provisional UnitSet
226.jats | core | 226.md | materializer,unitizer | docs | UTF8 JATS and verified raw/mapped coordinates
259 | core | 256,257,258 | projection | index | planner uses shared schema and verified UnitSet
260 | core | 259,331 | publication | publication | one submit/restore prepared-generation validator
261.model | core | 260 | publication | publication | closed transition/effect/readback model
261.control | core | 261.model,269 | publication,control,daemon | control | real journal/current-pointer adapter
262 | core | 256,258,327 | bridge | index | private schema/route/codec translation only
310 | core | 308,332,334,262,269 | secrets,protocol,bridge,supervisor,daemon | crypto | actual distinct provider/Qdrant secret role wiring
263 | core | 261.control,262,310,272 | bridge,publication,daemon | index | live exact effects/readback/query with one IDF population
329 | cleanup | 256,259,262 | point,projection,bridge,daemon | index | delete last legacy identity callers and minting
287 | core | 194 | access | control | validated grant scope/budget/disclosure intersection
288 | core | 287,272,258 | access | index | coherent scope/route/eligibility proofs
274 | core | 288,269 | access,daemon | native | live security before work and output
300 | core | 237,258,261.model | pins | lifecycle | owner-issued route/epoch pins and expiry/replay
282 | core | 287,300 | handles | canonical | exact source/range-bound handle core
283 | core | 282 | handles | lifecycle | atomic handle issue/expiry/revocation and recovery
275 | core | 283,274,272,330 | handles,daemon | control | actual authorized range-bound handle serving
284 | core | 287,300,272 | continuation | canonical | exact session/scope/profile paging admission
285 | core | 284 | continuation | lifecycle | atomic emission and terminal cleanup/recovery
276 | core | 285,275 | continuation,pins,daemon | control | real paging/pins; retire daemon-local catalogue
249 | core | 248,272,274,275,330 | exact,daemon | match | frozen denominator and exact witness handles
213 | core | 258 | contracts,domain,protocol | canonical | two additive recipe schemas; preserve eleven existing v1 recipes
278 | core | 213,258 | executor,planner,comparator | rank | pure exact arithmetic and closed leg/evidence model
290 | core | 288,278,213 | planner | rank | opaque budget-complete plans
291 | core | 290 | executor | rank | one request ledger and terminal outcome per leg
293 | core | 288,330,257 | validator | canonical | verify exact readback bytes and source facts
294 | core | 293,278,275 | projector | rank | deterministic validated projection/quotas/coverage
296 | core | 272,257 | resolver | facts | owner-derived resolution with honest ambiguity
297 | core | 296,272 | comparator | facts | exact source/lineage comparison observations
298 | core | 297,278 | comparator | rank | derived comparison matrix and recommended reading
277 | core | 213,249,263,274,275,276,333,252,254 | contracts,domain,planner,protocol,daemon | control | one operational capability registry
279 | core | 277,290,291,293,294,263,278 | planner,executor,validator,projector,daemon | rank | actual scope-to-result serving pipeline
280 | core | 279,296,298,332 | protocol,daemon,cli | cli | eleven canonical recipes on real provider
223 | core | 257,331 | facts | facts | pinned Tree-sitter queries and one fact batch
224 | core | 223 | facts | facts | SCIP importer plus one precise producer
232 | core | 223,213,278 | resolver,projector,comparator | rank | bounded deterministic source-backed orientation
281 | core | 280,232 | protocol,daemon,projector | rank | actual retrieve_evidence/orient_scope serving
336.core | core | 274,272,246 | overlay,daemon | canonical | coherent TTL/digest/budget/recovery/authorization
336.lsp | extension | 336.core,332,281 | lsp,client,overlay | agent | full-sync memory-only optional LSP leaf
228 | core | 269,272,330 | registry,daemon | connector | atomic checkpoint/event/failure/retry core
229 | core | 228 | registry,daemon | connector | dated OpenAlex snapshot import
230 | core | 228 | registry,daemon | connector | dated S2 release import, independent provenance
231 | core | 228,327,308 | registry,daemon | connector | bounded Crossref/OpenCitations live observations
227 | core | 266,308,331 | doc_worker,daemon,materializer | doc_worker | isolated worker, parent validation, total limits/cleanup
216.html | extension | 227,331 | doc_worker,materializer,unitizer | doc_worker | inert Tika HTML profile, no renderer or false offsets
216.latex | extension | 223,257,331 | facts,materializer,unitizer | facts | inert Tree-sitter LaTeX spans, no TeX execution
216.pdf | extension | 227,331 | doc_worker,materializer,unitizer | doc_worker | one isolated embedded-text PDF provider
216.scholar | extension | 227,331 | doc_worker,materializer,unitizer | doc_worker | GROBID scholarly PDF/TEI with region provenance
216.office | extension | 227,331 | doc_worker,materializer,unitizer | doc_worker | separate qualified Docling Office format profiles
216.legacy | extension | 227,331 | doc_worker,materializer,unitizer | doc_worker | isolated allowlisted Tika legacy formats
216.ocr | extension | 227,331 | doc_worker,materializer,unitizer | doc_worker | OCRmyPDF/Tesseract lossy text with original regions
225 | extension | 223,266 | facts,daemon | facts | bounded external Ctags heuristic fallback
236.import | extension | 223 | facts | facts | one SARIF importer reconciled to retained source
236.providers | extension | 236.import,266,330 | facts,daemon | facts | CodeQL/Semgrep findings, no custom dataflow engine
219 | extension | 281,332,275,276 | mcp,eliot_adapter,client | agent | tools-only MCP over the real shared provider
301 | core | 269,300 | retention | lifecycle | durable retention leases and owner time
304 | core | 269,272,274 | retention,access | lifecycle | restrictive purge state/tombstones/evidence
302 | core | 330,331,301,304 | retention,revision,daemon | lifecycle | real closed-root exact-ID CAS mark/sweep
303 | core | 263,300,304 | reclaimer,bridge,publication | lifecycle | retired-point reclaim with publication/pin permits
305 | core | 304,302,303,275,276,279 | retention,access,daemon | lifecycle | purge across real serving/storage planes
243 | core | 302,331 | backup,revision,control | backup | verified OCI backup; hostile import only to quarantine
244 | core | 243,308,330,269,305 | restore,control,revision,daemon | backup | same-owner/key fresh-root restore and guarded cutover
245 | core | 244,309,307 | restore,secrets,control,daemon | backup | per-object re-encryption/reference rewrite/new-key cutover
321 | cleanup | 315,331,263,333,305,245 | daemon | canonical | retire remaining daemon preimage exceptions by owner schema
233 | tooling | 237,213 | eval | eval | immutable topics/qrels/results/coverage/metrics
234.schema | tooling | 233 | eval,xtask | evidence | statement/subject/verifier implementation
240.runner | tooling | 233,234.schema,235,332,334 | runner,eval | eval | real installed provider/process/raw measurement runner
214 | cleanup | 251 | xtask,docs | cargo | remove controller-only commands, preserve product validators
188 | cleanup | 214 | xtask,docs | cargo | no mandatory Python/Node product/release validator runtime
220.A | docs | - | docs | canonical | preserve exact old normative bytes/hash
220.B | docs | 220.A,213,258 | docs | canonical | explicit new normative amendment/hash
139 | acceptance | 281,336.core,214 | model,model_worker,eliot_adapter,research_export,doc_worker | agent | closed implemented/qualified/shipped profile inventory
242.tool | tooling | 250,327 | xtask,docs | package | offline current-user installer and SBOM tooling
242.candidate | acceptance | 242.tool,281,305,245,321,188,139,335,229,230,231,226.jats,224,325,220.B | xtask,docs | package | frozen exact selected code/research candidate before installed tests
264 | acceptance | 263,329,281,310,330,331,336.core | daemon,bridge | index | fresh-generation integration/epoch/auth/recovery evidence
215.plan | docs | 233 | docs,eval | eval | cases/qrels/limits/platforms fixed before results
215.run | acceptance | 215.plan,242.candidate,240.runner,264,324 | runner,eval,docs | eval | installed core and all shipped-profile evidence
234.review | acceptance | 234.schema,215.run | eval,docs | evidence | independent semantic verification of immutable runs
137 | acceptance | 234.review | eval,docs | eval | final resources/disclosure/quality review
140 | release | 137 | docs | package | explicit human publication of exact tested bytes
```

The default target above includes local code/Git, structured preparation, exact/indexed recipes and research observations. If an accepted release profile intentionally ships a smaller subset, document its exact nonshipped features before testing; never silently remove a prerequisite after a failure. Optional extensions are implemented one profile at a time and add their D1/C1/A1 evidence to the selected package. Full programme progress still records unfinished extensions honestly.

## 5. Exact source entrypoints and integration boundaries

| Owner key | Existing path or declared new path |
|---|---|
| contracts / domain / ports / config | `crates/search-contracts`, `crates/search-domain`, `crates/search-ports`, `crates/search-config` |
| control / protocol / lexical / eval | `crates/search-control-redb`, `crates/search-provider-protocol`, `crates/search-lexical`, `crates/search-eval` |
| admission / identity / registry / reconcile / reader / revision | `crates/search-source/search-source-admission`, `search-source-identity`, `search-source-registry`, `search-source-reconcile`, `search-safe-reader`, `search-revision-store` under the same `crates/search-source/` prefix |
| materializer / unitizer / facts | `crates/search-prep/search-materializer`, `crates/search-prep/search-unitizer`, `crates/search-prep/search-code-enricher` |
| point / projection / bridge / supervisor | `crates/search-index-qdrant/search-point-identity`, `search-projection-planner`, `search-qdrant-bridge`, `search-qdrant-supervisor` under that prefix |
| publication / pins / reclaimer | `crates/search-index-qdrant/search-publication`, `search-epoch-pins`, `search-index-reclaimer` under that prefix |
| access / handles / continuation / overlay / exact | `crates/search-query/search-access`, `search-handles`, `search-continuation`, `search-overlay`, `search-exact` under that prefix |
| planner / executor / validator / projector | `crates/search-query/search-query-planner`, `search-retrieval-executor`, `search-candidate-validator`, `search-result-projector` under that prefix |
| resolver / comparator | `crates/search-query/search-subject-resolver`, `crates/search-query/search-comparator` |
| runtime / secrets / secrets_windows / revision_crypto / retention | `crates/search-runtime/search-runtime-owner`, `search-os-secrets`, `search-os-secrets-windows`, `search-revision-crypto`, `search-retention` under that prefix |
| model / eliot_adapter / research_export | `crates/search-model-provider`, `crates/search-eliot-adapter`, `crates/search-research-export-adapter` |
| daemon / cli / doc_worker / model_worker | `bins/eliot-searchd`, `bins/eliot-search`, `bins/eliot-search-doc-worker`, `bins/eliot-search-model-worker` |
| xtask / docs | `xtask`, `docs` |
| client / backup / restore / runner | Declared by their issues: `crates/search-provider-client`, `crates/search-backup`, `crates/search-restore`, `crates/search-eval-runner`; verify existing tree before creating. |
| mcp / lsp | Declared optional leaf binaries `bins/eliot-search-mcp`, `bins/eliot-search-lsp`; actual accepted package naming is recorded before addition. |

All 48 existing workspace members are covered, including the actual `search-revision-crypto` package. `search-ports` is an existing shared-input contract, not an independent rewrite. Proposed packages are created only if the assigned issue establishes a real boundary. Never create an imagined `search-qdrant-bridge-async` package.

Exact issue-scoped root/cross-package integration is permitted only to connect the named existing owner and its directly affected callers. Update corresponding package instructions in the same PR when necessary. The table does not grant permission to alter unrelated package semantics. The manager records and reviews the file list before editing.

## 6. Donor mechanisms, official documentation and rejection rules

| Family | What to reuse / code not to write | Primary references |
|---|---|---|
| canonical | Retain accepted length-first codec and private BLAKE3/RustCrypto; no second writer or digest catalogue. | [RFC8949](https://www.rfc-editor.org/rfc/rfc8949.html#section-4.2.3), [BLAKE3](https://github.com/BLAKE3-team/BLAKE3), [RustCrypto](https://github.com/RustCrypto/hashes) |
| control | Existing redb transactions and technical records; not source bodies or searchable FTS. | [redb](https://docs.rs/redb/), [source](https://github.com/cberner/redb) |
| native | Existing Win32 handle/Job/identity adapters; interprocess only as accepted private I/O, never authentication. | [Job objects](https://learn.microsoft.com/en-us/windows/win32/procthread/job-objects), [pipe security](https://learn.microsoft.com/en-us/windows/win32/ipc/named-pipe-security-and-access-rights), [interprocess](https://github.com/kotauskas/interprocess) |
| crypto | RustCrypto AEAD, zeroize and Windows credentials; ELIOT owns purpose/reference/nonce lifetime, not a custom cipher. | [AEADs](https://github.com/RustCrypto/AEADs), [zeroize](https://docs.rs/zeroize/), [WinCred](https://learn.microsoft.com/en-us/windows/win32/api/wincred/) |
| config / cli | Maintained TOML + bounded typed Serde, lexopt, JSON edge. No extra config layering/runtime. | [TOML](https://github.com/toml-rs/toml), [Serde](https://serde.rs/impl-deserialize.html), [lexopt](https://github.com/blyxxyz/lexopt), [serde_json](https://docs.rs/serde_json/) |
| match / glob | memchr, Aho-Corasick, regex-automata, GlobSet over canonical bytes. No custom automata, hidden transforms or filesystem walker. | [memchr](https://github.com/BurntSushi/memchr), [Aho-Corasick](https://github.com/BurntSushi/aho-corasick), [regex-automata](https://docs.rs/regex-automata/), [GlobSet](https://docs.rs/globset/) |
| unicode | XID+NFC for code; accepted #253 C+F/T **without normalization** for prose folding. Preserve distinct profiles and original ranges. | [UAX31](https://www.unicode.org/reports/tr31/), [UAX15](https://www.unicode.org/reports/tr15/), [UAX29](https://www.unicode.org/reports/tr29/) |
| git / watch | Patched narrow gitoxide; notify/native hints with mandatory gap on loss/restart. No clone, checkout, hooks or watcher-silence proof. | [Gitoxide](https://github.com/GitoxideLabs/gitoxide), [advisories](https://github.com/GitoxideLabs/gitoxide/security/advisories), [notify](https://github.com/notify-rs/notify), [ReadDirectoryChangesW](https://learn.microsoft.com/en-us/windows/win32/api/winbase/nf-winbase-readdirectorychangesw) |
| index / publication | Existing Qdrant only; shared schema/population; Lucene immutable commit-point invariant, not Lucene/Tantivy backend. | [Qdrant hybrid](https://qdrant.tech/documentation/concepts/hybrid-queries/), [security](https://qdrant.tech/documentation/guides/security/), [Lucene IndexWriter](https://lucene.apache.org/core/10_3_1/core/org/apache/lucene/index/IndexWriter.html) |
| facts | Tree-sitter grammar/query pair; SCIP and existing language indexers; Glean/Kythe fact/anchor patterns; optional Ctags/SARIF. No semantic compiler per language. | [Tree-sitter](https://tree-sitter.github.io/tree-sitter/), [SCIP](https://github.com/sourcegraph/scip), [Glean](https://github.com/facebookincubator/Glean), [Kythe](https://kythe.io/docs/schema/), [Ctags](https://docs.ctags.io/), [SARIF](https://docs.oasis-open.org/sarif/sarif/v2.1.0/) |
| docs | pulldown-cmark events and quick-xml pull parser. Offset claims must be verified; no rendering/entity execution. | [Markdown](https://docs.rs/pulldown-cmark/), [quick-xml](https://docs.rs/quick-xml/), [JATS](https://jats.nlm.nih.gov/) |
| doc_worker | Tika process/memory/time isolation; one selected PDF provider, GROBID, Docling/Tika, OCRmyPDF/Tesseract. No in-daemon hostile parser. | [Tika security](https://tika.apache.org/docs/4.0.x/security.html), [LiteParse](https://github.com/run-llama/liteparse), [PDFBox](https://pdfbox.apache.org/), [GROBID](https://grobid.readthedocs.io/), [Docling](https://docling-project.github.io/docling/), [OCRmyPDF](https://ocrmypdf.readthedocs.io/) |
| connector | Zotero/Onyx checkpoints/tombstones/retries; dated OpenAlex/S2 and bounded Crossref/OpenCitations observations. No provider-specific catalogue. | [Zotero sync](https://www.zotero.org/support/dev/web_api/v3/syncing), [Onyx interfaces](https://github.com/onyx-dot-app/onyx/blob/main/backend/onyx/connectors/interfaces.py), [OpenAlex](https://help.openalex.org/access/snapshot/), [S2](https://api.semanticscholar.org/api-docs/datasets), [Crossref](https://www.crossref.org/documentation/retrieve-metadata/rest-api/), [OpenCitations](https://opencitations.net/) |
| rank | Vespa staged-ranking concept, Aider compact orientation; exact product arithmetic and source-backed comparison. No LLM hot path. | [Vespa](https://docs.vespa.ai/en/phased-ranking.html), [Aider](https://github.com/Aider-AI/aider/blob/main/aider/repomap.py) |
| lifecycle / backup | Nix reachability/roots, OCI layout, bounded tar/zstd transport; ELIOT retains purge/restore/key/cutover authority. | [Nix GC](https://nix.dev/manual/nix/stable/command-ref/nix-store/gc), [OCI layout](https://github.com/opencontainers/image-spec/blob/v1.1.1/image-layout.md), [tar](https://docs.rs/tar/), [zstd](https://docs.rs/zstd/) |
| eval / evidence | Anserini/Pyserini/BEIR run/qrels, raw-backed HdrHistogram, in-toto/DSSE envelope. Native counters authoritative; no self-approval. | [Anserini](https://github.com/castorini/anserini), [Pyserini](https://github.com/castorini/pyserini), [BEIR](https://github.com/beir-cellar/beir), [HdrHistogram](https://docs.rs/hdrhistogram/), [in-toto](https://github.com/in-toto/attestation), [DSSE](https://github.com/secure-systems-lab/dsse) |
| package / agent / cargo | Offline current-user package generator+CycloneDX; official MCP/LSP at optional leaves; Cargo metadata/Syn for developer facts. | [cargo-packager](https://github.com/crabnebula-dev/cargo-packager), [CycloneDX Rust](https://github.com/CycloneDX/cyclonedx-rust-cargo), [MCP Rust](https://github.com/modelcontextprotocol/rust-sdk), [LSP](https://microsoft.github.io/language-server-protocol/), [cargo_metadata](https://github.com/oli-obk/cargo_metadata), [Syn](https://github.com/dtolnay/syn) |

Existing donor snapshots #207/#209/#210/#200 are read-only model/fixture references. Current accepted code is the working base. A donor version alone is not acceptance: record archive checksum, tag/commit, license/data terms, MSRV, normal/build/dev closure, features, advisories and the exact API/resource behavior. Never infer latest release safety or silently widen dependencies. Complex optional profiles may be rejected; a failed candidate is not permission to build a framework.

## 7. Programme / old PR disposition

| Programme or old PR | Current executable owner or closing condition |
|---|---|
| #97 | Coordination and current progress only; its historical branch never merges. |
| #109 | #238 parser → #333 runtime apply/readiness. |
| #111 | #309 crypto + #330 real protected revision store. |
| #112 / #113 | #316 digest + #257 UnitSet + #331 actual preparation; profiles separately #226/#227/#216. |
| #115 | #307 → #308 → #309, then #310 role wiring. |
| #116 | #235 typed extraction → #332 native/CLI serving cutover. |
| #119 / #120 / #121 | #334 process; #262/#263 vendor plane; #264/#215 qualification. |
| #127 / #185 | Integrated acceptance under #264/#215, not implementation branches. |
| #129 / #130 | #335 read-only Git; #336 overlay core then optional LSP. |
| #132 | #223/#224 fact plane; optional #225/#236. |
| #186 / #190 | #262/#263 vendor isolation, no second adapter. |
| #187 / #191 | #250 delivered; #251 source-shape checks; no handwritten Cargo resolver. |
| #184 / #189 / #193 | Actual owner cutovers/deletion, not mechanical module splitting first. |
| #194 / #201 | Durable binding under control; actual Qdrant auth handoff through #310 with correct/missing/wrong-key evidence. |
| #205 | Contract is delivered by #258; #220 amendment and #260/#263/#264 exhaustion/rendering evidence still required. |
| #213 / #221 | Public schemas; pure #278 then actual #279/#232/#281 integration. No competing ranking engine. |
| #215 | Named plan/run acceptance phases; no product implementation owner. |
| #216 | Independent named format phases above, shipped list under #139/#242. |
| #218 | #266–#272 authority and #228–#231 research connectors. |
| #220 | Exact old Part I/hash extraction, then separate versioned normative amendment. Issue numbers do not become normative wire schema. |
| #137 / #140 | Final evidence review and explicit human release of the exact tested bytes. |

Old closed competing/packet branches remain closed. A tracking PR is never counted as a pending source merge. Do not create another implementation issue where a named current owner exists.

## 8. Reusable bounded subagent tasks

For each current slice, assign 5–10 of these narrow read-only reviews: authority/current callers; donor version/API; minimal file change/deletion list; persistence/security/recovery; allocation/deadline/cancellation; profiles/coordinates/compatibility; exact acceptance commands/cases; independent final diff review.

Every response contains path+symbol and source link, concrete defect/change, evidence and stop/accept implication. No open-ended 'audit the whole project again' report. Manager verifies load-bearing claims and integrates. Reviews are source reviews, not self-issued release qualification.

Manager handoff: issue/phase, base/result SHA, exact files and reverse callers, donor versions/features/checksums/licenses/advisories, canonical/profile impact, legacy sites and deletion owner, check/Clippy output, targeted cases written/executed, observed failures, and the next ready task. Keep the existing canonical guard ledger; no second ledger/controller.

## 9. Final acceptance and limits of this audit

Core source completion requires the actual admitted-source → immutable protected revision → verified preparation → publication → authorized scoped retrieval → exact readback → deterministic range-bound output path. Models, booleans, interfaces or documentation do not satisfy this. Every pending authority defect in the shipped closure needs a real owner fix; source-code presence cannot close integration.

Qualification uses immutable package/config/corpus/query/case/SLO identities and actual Windows processes. Required cases cover provision/start, all claimed recipes, permissions/IDF noninterference, source/currentness, crash/restart/rebuild, pins/handles/continuations, purge/reclaim, backup/restore/key migration, total resources, no-write/disclosure, offline install, upgrade/rollback-or-refusal and uninstall-preserve-data. Failures and unrun cases remain in denominators. Independent review follows execution, then human publication of identical final bytes.

Optional profiles are listed explicitly as shipped-qualified or not shipped. Not shipped does not close their implementation phases. The project map is complete through these decisions; it does not claim all remaining code is correct, every donor universally optimal, any new Cargo/native run performed here, or the product release-ready.
