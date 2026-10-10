# ELIOT Search: complete implementation and release map

**Audit base:** `1a6d17d073c5f6d1fe50f21082439f1965d24ef8`. **Coordinator:** [#97](https://github.com/UnknownAlienHuman/eliot-search/pull/97). **Topology:** one integration manager, one writer worktree, 5–10 read/research/review subagents. **Product acceptance:** not established by this document.

This is the whole-project continuation, not another wave or product subsystem. It supersedes old scheduling prose in dated audits and packet PRs. It does not replace Architecture Part I, accepted ADRs, package contracts, detailed issue semantics or actual execution evidence. No runtime consumes this document.

## 1. Start and stop rules

Read [root instructions](../../AGENTS.md), [architecture entrypoint](../architecture/README.md), [ADR 0005](../adr/0005-standalone-search-product-and-controller-boundary.md), [ADR 0006](../adr/0006-agent-analysis-framework-product-scope.md), this map, the exact issue, nearest package AGENTS/FUNCTIONS, then current code. [Package status](PACKAGE_STATUS.toml) describes packages; this map describes work. Neither grants source/access authority to runtime.

Completed implementations: #317/#318 tooling prerequisite; #237/#319 canonical foundation; #253/#322 casefold research; #258/#323 indexed contracts; #250/#326 Cargo metadata. Do not repeat them. Reported scoped compiler/fixture results are not whole-product qualification. #253 completed a decision, not #254's tokenizer.

**Next coding task remains #256.** Do not interrupt an active manager, reset its worktree or silently change its accepted base. A docs-only merge does not invalidate code evidence, but the next source slice records its actual base. #324 remains a real validator rerun obligation; #327 remains a dependency/advisory obligation, not a reason to restart foundation work.

For each source slice: inspect actual callers, implement a coherent change, compile affected packages and immediate reverse consumers, run strict Clippy, retain the exact output, review, then merge. Broad installed/native/scale tests stay in final qualification; tiny causal checks are allowed when necessary. No workflow-trigger proliferation. No global formatting or speculative framework work.

The manager alone changes manifests/Cargo.lock and integrates. Subagents return concrete paths/symbols, evidence, proposed changes and blockers; they do not write shared state. Reviews are independent of the subagent that proposed the change. A review label is not execution evidence.

## 2. Stage semantics: remove false dependency cycles

The table below is an executable completion DAG. `N.phase` is a phase of existing issue/PR N, **not a new GitHub issue**. Prerequisites mean required artifacts/interfaces for that phase, not that every optional consumer must already exist. `-` means no unmet predecessor inside this map; root/package rules still apply.

- **Configured** means requested configuration parsed and accepted.
- **Operational** means constructed current owners, profiles and readbacks can perform the operation.
- **ReleaseQualified** means an exact immutable candidate passed independent external acceptance.

The product must be operational before its installed qualification can run. Product startup cannot depend on its own future benchmark/report. Evaluation/attestation tools must not become product dependencies.

Similarly: packaging-tool implementation precedes a candidate; a candidate precedes installed runs; verification of those runs precedes release. A candidate is allowed to be unqualified. No rebuild or signing mutation after final accepted runs.

Models and integration are separate: #235.core can be extracted while preserving existing external assembly; #332 replaces that assembly. #277.schema is usable before #277.live. #234.build implements the verifier; #234.review uses it after runs. Pure retention ports can exist before restore, but destructive release is forbidden until complete root/reference inventories include every live producer.

## 3. Whole-project task DAG

Columns are stable: stage, prerequisites, owning implementation area, exact exit artifact/result. Existing detailed issues contain fixtures and local APIs; this table removes orchestration ambiguity. All numeric references resolve to this repository. `integration` is an aggregate checkpoint, not a new executor or database.

| Stage | Prerequisites | Owner / source entrypoint | Exit result |
|---|---|---|---|
| 237 | - | search-contracts; merged #319 | Bounded canonical bytes and real digest APIs; retained exceptions explicit |
| 253 | - | Research; merged #322 | Exact full-fold decision and goldens, not production tokenization |
| 258 | 237 | search-contracts; merged #323 | One indexed payload/index/eligibility/epoch contract |
| 250 | 237 | xtask; merged #326 | Bounded Cargo graph/status tool |
| 324 | 250 | Existing xtask validator | Rerun on fixed docs; retain actual outcome, never edit predicate to pass |
| 327 | - | Manager dependency lane | Accepted patched dependency and transitive/advisory record |
| 256 | 237,258 | search-point-identity | New stateless S11; old required callers preserved unchanged until 329 |
| 257 | 237,258 | search-unitizer | Internally verified complete UnitSet v3 |
| 266 | 237 | search-runtime-owner; daemon owner_composition | Distinct inspect/mutate/init/recovery capabilities; no create-on-read |
| 235.core | 237 | New search-provider-client; typed CLI source | Single typed session engine; no new TCP/token fallback; existing assembly unchanged |
| 238 | 237 | search-config | Maintained TOML; bounded typed conversion; honest v1/v2 digest migration |
| 241 | 237 | search-source-admission | Every field effective; explicit GlobSet profile; safety deny dominates |
| 246 | 237 | search-exact/literal | Mature matcher; existing overlap/chunk/ASCII semantics preserved |
| 252 | 237 | search-lexical | Internally identified XID+NFC code profile and original source ranges |
| 251 | 250 | xtask source lint | syn AST replaces pseudo-parser; compiler-required cases remain explicit |
| 214 | 250,251 | Residual controller tooling | Remove historical issuance/lease runtime; preserve useful source validators |
| 315 | 237 | Daemon sha256 and actual callers | Shared algorithm execution, exact legacy framing classified |
| 316 | 237 | Materializer digest/profile helpers | Shared compute; representation rebuild for changed identity bytes |
| 239 | 266 | Watch adapter and currentness hint boundary | Bounded events; all detail loss opens gaps; no watcher-issued currentness |
| 267 | 266,241 | search-safe-reader; native adapter | RootRecord/final-handle/location/live-security proof, not parent-path hints |
| 268 | 266 | Control migration; search-control-redb | Complete accounted legacy inventory and verified staged target |
| 269 | 268 | Redb/daemon control composition | Atomic sole-authority cutover; no dual write or fallback |
| 307 | 237,269 | search-os-secrets | Durable purpose/selection/candidate/retiring model and finite leases |
| 308 | 307,266 | search-os-secrets-windows | Real current-user backend effects, readbacks, unknown-outcome recovery |
| 309 | 307,308 | Revision crypto boundary | Key-reference-bound AEAD, nonce/lifetime policy, authenticated readback |
| 330 | 266,269,309 | search-revision-store | Real residency-bound immutable storage and bounded reference inventory |
| 331 | 257,316,330,269 | Materializer/unitizer; daemon preparation_store | One persisted verified representation/maps/UnitSet bundle; no zero-success fallback |
| 270 | 241,267,269,330 | Source ingestion/identity/registry | Pre-admit before bytes, one identity index, bounded staging and durable commit |
| 271 | 239,267,269,270 | search-source-reconcile; workspace composition | Immutable coherent/partial SourceView after final all-root revalidation |
| 272 | 269,271 | Source registry/corpus/portfolio owner | Append-only revisions, guarded aliases, exact authorized population |
| 129 | 267 | Git source adapter | Read-only patched Git plumbing, bounded pack/delta, no network/checkout |
| 333 | 238,266,269 | Daemon config_composition and control snapshot | Durable applied config, operational readiness without release-report dependency |
| 213.schema | 258 | Existing public contract owners | Preserve eleven v1 recipes; freeze retrieve_evidence@1/orient_scope@1 |
| 287 | 213.schema,258 | search-access | Grants bound to requested scope, budget and disclosure ceilings |
| 288 | 287,272,258 | search-access | Non-forgeable scoped route and eligibility plan |
| 194 | 269,272,287 | Redb binding/grant records and pairing producer | One durable coherent binding/grant-policy snapshot; no daemon invented authority |
| 332 | 235.core,266,194,308 | CLI; daemon provider/listener/entry | One native endpoint, invocation parser and JSON edge; remove old product fallback |
| 120.native | 266,327 | search-qdrant-supervisor | Exact owned child/Job/native identity and finite lifecycle; auth wired by 310 |
| 310 | 307,308,332,120.native | Secret consumer integration | Separate provider/Qdrant purpose-bound selected leases |
| 259 | 256,257,258 | search-projection-planner | Planner consumes verified UnitSet, one S11 and schema; no caller completeness |
| 260 | 259 | search-publication | Same internally recomputing prepared-generation validator for submit/recovery |
| 261 | 260,269,331 | Publication/control/store adapters | Durable intent, exact effects/readback, guarded current pointer and crash recovery |
| 262 | 256,258,327 | search-qdrant-bridge | Private vendor codec/schema/route; one eligible population; no duplicated identity |
| 263 | 261,262,272,310 | Live Qdrant/owner composition | Real generation writes/readback/scoped retrieval/IDF under one operation deadline |
| 329 | 259,262 | Identity consumers and old exports | Delete legacy registry/FNV minting/callers after consumers compile against S11 |
| 300 | 237,258 | search-epoch-pins | Owner-issued route/epoch/time pins; cannot be forged by callers |
| 282 | 237,300 | search-handles | Canonical bounded ranges, identity and invalidation model |
| 283 | 237 | search-continuation | Exact IDs, times and limits |
| 284 | 283,300 | search-continuation | Real pin lifecycle and expiry |
| 285 | 284 | search-continuation | Atomic emission, terminal cleanup and restart recovery |
| 274 | 288,194,333 | Access integration | Owner-issued live security checkpoints and restrictive revalidation |
| 275 | 282,274,330 | Handle integration | Exact retained-range expansion with current authorization |
| 276 | 285,274,275 | Paging integration | One continuation authority; no caller cursor or legacy catalogue bypass |
| 247 | 246 | search-exact | Bounded multi-literal profile through mature automata |
| 248 | 247 | search-exact | Explicit finite-automata regex profile; no backtracking fallback |
| 249 | 248,272,267,275 | Exact executor/readback | Frozen denominator and complete-negative proof; missing/denied/gap not absence |
| 254 | 252,253 | search-lexical | Exact versioned prose profile, full fold/range maps and query/document parity |
| 223 | 257,331 | search-code-enricher/profile registry | Pinned Tree-sitter syntax facts; retire custom parser only after caller cutover |
| 224 | 223 | Precise fact importer | Streaming SCIP producer artifact reconciled to exact retained source |
| 225 | 223 | Optional symbol provider | Pinned Ctags sidecar; heuristic assurance only |
| 236 | 223 | Optional finding importer | One bounded SARIF importer; CodeQL/Semgrep providers do not invent identities |
| 278 | 237,213.schema | Ranking core | Closed leg/profile table, checked exact arithmetic, stable complete tie order |
| 290 | 288,278,213.schema | search-query-planner | Opaque plan with complete finite work budget |
| 291 | 290 | search-retrieval-executor | Shared request budget and one terminal outcome per leg |
| 293 | 288,263,330,331 | search-candidate-validator | Validate actual readback bytes/source/profile, never caller digest equality alone |
| 294 | 278,293,275 | search-result-projector | Validated ranked cards, truthful coverage, bounded output |
| 296 | 272,223,293 | search-subject-resolver | Owner-derived resolution ladder with explicit ambiguity/coverage |
| 297 | 272,293 | search-comparator | Verified portfolio/candidate/lineage observations |
| 298 | 297,296,278 | search-comparator | Derived comparison matrix/reading/coverage; no caller verdict |
| 232 | 296,278,223 | Orientation composition | Deterministic Aider-inspired bounded map with exact source handles |
| 277.schema | 213.schema,258 | Capability contract | One closed recipe descriptor and availability vocabulary |
| 277.live | 277.schema,333,249,263,274,275,276 | Runtime capability composition | Actual owner-derived registry; optional profiles individually absent/unavailable |
| 279 | 290,291,293,294,277.live | Daemon query composition | One served live query path, not a parallel handler tree |
| 280 | 279,332 | Provider recipe host | Existing eleven v1 recipes routed through canonical owners |
| 281 | 280,232,298 | Provider evidence/orientation host | Two new versioned recipes, no find_text overloading |
| 226.markdown | 257,331 | Materializer/unitizer | Inert pulldown-cmark source events, verified range maps and complete UnitSet |
| 226.jats | 226.markdown | Same owners | quick-xml/JATS, no entity/network execution; decoded text explicitly mapped |
| 227 | 266,331,308 | Document worker/supervisor | Generic bounded isolated runtime validated first with a synthetic worker |
| 216.html | 227 | Isolated document adapter | Selected Tika HTML parser, inert content and honest mapped coordinates |
| 216.latex | 223,331 | Structural profile | Pinned tree-sitter-latex, inert syntax; no TeX execution/include expansion |
| 216.pdf | 227 | Isolated PDF adapter | One accepted LiteParse/PDFium or named PDFBox fallback, no silent OCR |
| 216.scholar | 227 | Scholarly document adapter | Pinned GROBID, TEI/page/bbox reconciliation and bibliography observations |
| 216.office | 227 | Office adapter | Docling DOCX/XLSX/PPTX separately identified; macros/resources never executed |
| 216.legacy | 227 | Legacy-format adapter | Explicit Tika parser allowlist, no broad discovery/external-command activation |
| 216.ocr | 227 | OCR adapter | OCRmyPDF/Tesseract, pinned language packs; lossy region-bound text |
| 228 | 269,272,330 | Connector core | Bounded checkpoint/events/tombstones/retry/gaps committed with retained observations |
| 229 | 228 | OpenAlex adapter | Exact snapshot records, no latest/title identity |
| 230 | 228 | Semantic Scholar adapter | Pinned release; distinguish paper/corpus identifiers |
| 231 | 228,308,327 | Crossref/OpenCitations adapters | Origin-locked APIs, bounded retries/deadline; contradictory observations retained |
| 301 | 269,300 | search-retention | Durable leases and journaled lifecycle without capacity leak |
| 302 | 301,330,331,261 | Retention/CAS inventory | Closed reachability roots, bounded preview and exact sweep plan |
| 303 | 300,261,263,329 | Indexed reclaim | Committed-publication and pin-permit-bound exact retired-point reclaim |
| 304 | 269,274 | Purge core | Restrictive barrier/tombstone state first; typed evidence, no caller completion |
| 305 | 304,302,303,275,276,279 | Purge integration | All live source/index/query/handle planes narrow before deletion |
| 243 | 302,330,331 | Backup container | Verified OCI layout; hostile tar.zst only transport; complete raw manifests |
| 244 | 243,266,269,308,330,305 | Restore coordinator | Same-owner/same-key fresh quarantined root, exact readback and guarded cutover |
| 245 | 244,307,309 | Migration coordinator | Per-object decrypt/verify/re-encrypt, full reference rewrite, old copies retained |
| 134.release | 302,303,305,243,244,245 | Existing retention owner | Delete/release only after every live/backup/restore/key root is registered and rechecked |
| 233 | 237 | Pure search-eval formats | Immutable corpus/query/qrels/results/coverage/run model, deterministic metrics |
| 234.build | 233 | Tooling verifier | Subject/predicate/reviewer rules; signature is not semantic truth |
| 240.build | 233,234.build,235.core | Installed runner tooling | Bounded native process/raw timing/inventory/disclosure collector, no product dependency |
| 139.inventory | 223,224,225,236,216.html,216.latex,216.pdf,216.scholar,216.office,216.legacy,216.ocr,229,230,231 | Optional/profile programme | Every profile marked implemented/disabled/not-shipped; unresolved is not complete |
| 264 | 281,129,254,226.jats,329,305,310,331,332,333 | Integration gate | Real fresh-generation product, restart/unknown outcome/access/exact-readback evidence |
| 119.live | 263,120.native | Qdrant qualification | Exact server/client/config/schema/IDF/native binding accepted; no floating artifact |
| 220 | 258,256,257 | Normative documentation | Preserve old Part-I bytes/hash; issue reviewed amendments with new version/hash |
| 242.tool | 237 | Packaging tools only | Offline current-user package generator/cache/SBOM, no provisioning in installer |
| 242.candidate | 242.tool,264,119.live,245,134.release,220,324,327 | Package composition | Frozen closed signed candidate, licenses/SBOM/guide; not yet release-qualified |
| 215.plan | 233,242.candidate | Qualification plan | Exact corpus/queries/cases/resources/SLOs frozen before measured results |
| 215.run | 215.plan,240.build,242.candidate | Installed qualification | M1/W1/P1/P2/I1/R1/N1 plus every shipped profile tier; failures stay in denominator |
| 234.review | 234.build,215.run | Independent evidence reviewer | Reopen/recompute exact subjects; no author-written approved Boolean |
| 137.review | 234.review | Whole-product review | Review functionality/resources/security/disclosure and retained failures |
| 140.publish | 137.review,242.candidate,214 | Release programme | Explicit human publication of the exact tested bytes and operator guide |
| 130 | 235.core,332,274,275,276,279 | Optional LSP/overlay | Full-sync memory-only adapter; all overlay TTL/snapshot/recovery/reauth findings fixed |
| 219 | 235.core,281,332 | Optional MCP adapter | Dated tools-only leaf; no raw filesystem/Qdrant, independent source authority or model loop |
| 215.adapters | 130,219,240.build,234.build | Optional A1 acceptance | Separate exact shipped adapter candidate and independent acceptance; never inherit core PASS |

Rows for optional profiles are work through their existing programme, not permission to ship them unqualified. #139.inventory can run per profile early; an unavailable profile remains unfinished. A **core release** need not wait for all optional rows, but must declare each not-shipped profile. Claiming **the entire backlog complete** requires all selected optional phases and their acceptance. No baseline obligation from ADR 0006 may be dropped silently.

## 4. Work ordering and package seams

Keep the already accepted remaining Wave-2 order: 256 → 257 → 266 → 235.core → 238 → 241 → 246 → 252 → 226. The table's preparation/store prerequisites for **integrated completion** do not require redoing model/profile source work. In particular, #226 parser/profile code can be prepared after #257; its real persistence/query acceptance waits for #331. Record source-complete versus integrated instead of holding an impossible all-in-one PR.

After Wave 2, choose the first unfinished stage whose dependencies are actually delivered. Prefer closing a working vertical over building every optional adapter. Recommended progression: root/control → secrets/storage → ingestion/currentness/scopes → publication/vendor → access/handles/exact/query → profiles/connectors → lifecycle → installed candidate/qualification/release.

Shared changes are serialized: contracts first; control schema before consumers; typed clients before adapters; fact model before producers; worker ABI before parser workers; run formats before runner/verifier; restore ports before key migration. Cross-package edits are limited to a named interface/caller cutover. A phase requiring other files gets an explicit instruction addendum, not blanket permission.

Compatibility rule: do not remove a still-imported public API under a package-only PASS. Inventory all immediate reverse consumers. #256 introduces S11 without promoting old identities; #329 removes old callers only after #259/#262. Old data can have a named read-only decoder/rebuild path; it cannot silently mint new identities or serve as new-profile fallback.

Config digest rule: existing SHA256(legacy bytes) is not SHA256(domain || NUL || bytes). #238 may add only the reviewed bounded exact-byte SHA helper in the existing shared owner, using the same donor. Preserve old vectors or issue explicit version/rebuild. No second codec, crypto crate or generic receipt system.

## 5. Donor reuse contracts

A link is a reading entrypoint, not a claim that its moving HEAD is qualified. Use the existing exact accepted lock/profile when valid; otherwise verify an exact release, checksum, license, MSRV, normal/build dependencies, feature closure and advisories **once for that adoption**, then record the decision. Do not repeatedly research the entire ecosystem or downgrade/upgrade unrelated pins. Context7 summaries do not override exact release source.

| Owner/tasks | Mature donor / primary documentation | What to reuse; what remains local |
|---|---|---|
| 237,315,316,238 | [BLAKE3](https://github.com/BLAKE3-team/BLAKE3), [RustCrypto hashes](https://github.com/RustCrypto/hashes), [RFC8949](https://www.rfc-editor.org/rfc/rfc8949.html) | Algorithms only; keep accepted length-first canonical bytes and owner schemas |
| 238,333 | [TOML](https://github.com/toml-rs/toml), [Serde](https://serde.rs/) | Grammar/typed visitor; ELIOT owns precedence/secrets/apply, not a new config framework |
| 332,235 | [lexopt](https://github.com/blyxxyz/lexopt), [serde_json](https://docs.rs/serde_json/), [Windows pipe security](https://learn.microsoft.com/en-us/windows/win32/ipc/named-pipe-security-and-access-rights) | One parser/serializer/native adapter; trusted binding/auth/session stay local |
| 241 | [globset](https://github.com/BurntSushi/ripgrep/tree/master/crates/globset) | Explicit matcher settings and relative bytes; no walker/root authority |
| 246,247,248 | [memchr](https://github.com/BurntSushi/memchr), [Aho-Corasick](https://github.com/BurntSushi/aho-corasick), [regex](https://github.com/rust-lang/regex) | Matching engines; ELIOT keeps overlap/chunk/range/budget/exhaustiveness proof |
| 250,251 | [Cargo metadata](https://doc.rust-lang.org/cargo/commands/cargo-metadata.html), [cargo_metadata](https://github.com/oli-obk/cargo_metadata), [syn](https://github.com/dtolnay/syn) | Official dependency/AST facts, bounded subprocess; no pseudo-compiler |
| 252,253,254 | [UAX31](https://www.unicode.org/reports/tr31/), [UAX15](https://www.unicode.org/reports/tr15/), [UCD](https://www.unicode.org/ucd/) | Pinned data/normalization/folding; exact source mapping and profile identities |
| 129 | [gitoxide](https://github.com/GitoxideLabs/gitoxide) | Patched read-only object/pack/ref plumbing; no clone/fetch/checkout/credential/hooks |
| 223,224,225,236 | [Tree-sitter](https://tree-sitter.github.io/tree-sitter/), [SCIP](https://github.com/sourcegraph/scip), [Ctags](https://docs.ctags.io/), [SARIF](https://docs.oasis-open.org/sarif/sarif/v2.1.0/) | Parsers/protocols; no hand-written language indexers or second graph authority |
| 259–263 | [Qdrant](https://qdrant.tech/documentation/), [Lucene](https://lucene.apache.org/core/) | Existing vendor index and commit invariants; ELIOT source/manifest/access remain authoritative |
| 232,278 | [Aider map](https://aider.chat/docs/repomap.html), [Vespa phases](https://docs.vespa.ai/en/phased-ranking.html) | Bounded orientation and phased ranking; exact deterministic arithmetic, no runtime framework |
| 226,216 | [pulldown-cmark](https://docs.rs/pulldown-cmark/), [quick-xml](https://docs.rs/quick-xml/), [JATS](https://jats.nlm.nih.gov/) | Inert parser events, checked raw spans; no invented parser features/automatic exactness |
| 227,216.* | [Tika](https://tika.apache.org/), [GROBID](https://grobid.readthedocs.io/), [Docling](https://docling-project.github.io/docling/), [OCRmyPDF](https://ocrmypdf.readthedocs.io/) | Isolated per-profile parsing; no universal in-process hostile parser or runtime download |
| 228–231 | [Zotero sync](https://www.zotero.org/support/dev/web_api/v3/syncing), [Onyx](https://github.com/onyx-dot-app/onyx), [OpenAlex](https://docs.openalex.org/), [Semantic Scholar](https://www.semanticscholar.org/product/api), [Crossref](https://www.crossref.org/documentation/retrieve-metadata/rest-api/), [OpenCitations](https://opencitations.net/) | Checkpoints/raw observations/snapshots; never remote title or citation edge as source truth |
| 301–305,243–245 | [Nix manual](https://nix.dev/manual/nix/stable/), [OCI image spec](https://github.com/opencontainers/image-spec) | Reachability and container format; all deletion/key/cutover authority stays in ELIOT |
| 233,234,240,215 | [Anserini](https://github.com/castorini/anserini), [Pyserini](https://github.com/castorini/pyserini), [BEIR](https://github.com/beir-cellar/beir), [in-toto](https://github.com/in-toto/attestation), [DSSE](https://github.com/secure-systems-lab/dsse) | Reproducible artifacts/envelopes; actual effects and independent verdicts remain separated |
| 242,140 | [cargo-packager](https://github.com/crabnebula-dev/cargo-packager), [CycloneDX Rust](https://github.com/CycloneDX/cyclonedx-rust-cargo) | Offline generator/SBOM; installer copies program files only, product provisions state |
| 130,219 | [LSP](https://microsoft.github.io/language-server-protocol/), [MCP](https://modelcontextprotocol.io/specification/) | Exact dated protocol leaves over shared client; no second planner or database |

## 6. Completion criteria by vertical

**Source truth:** every normal path enters one typed root capability. Read-only inspection creates nothing. Pre-admission precedes reading; final native handles and current barriers are checked. Redb is sole technical authority. Immutable storage and preparation reopen and verify exact bytes; watcher quietness is not currentness.

**Indexed retrieval:** new S11, UnitSet, payload/schema and generation agree; retrieval/IDF/count/validation share the exact population. Submit and recovery use the same validator. Unknown effects remain recoverable. Qdrant is rebuilt from retained authoritative artifacts, not used to discover source truth.

**Useful public product:** real standalone CLI/provider supports the preserved eleven recipes and the two new versioned recipes. Exact scans prove only their frozen denominator. Candidate validation precedes citations and stable ranking. Handle/continuation expansion reauthorizes and cannot widen the original range. Orientation/comparison are source-backed navigation, not agent verdicts.

**Lifecycle:** retirement/restrictive purge changes visibility before reclamation. Leases, pins, operations, backups, restores, rollback copies and key generations appear in a complete bounded reachability graph. Absence/count/path/receipt alone never authorizes deletion. Same-key restore and new-key migration are separate recoverable transitions; old last copies/keys survive until exact owner release.

**Release:** normal startup has no Memory OS/controller dependency. Candidate installs as a standard Windows user offline, explicitly provisions state, uses actual named pipe/Qdrant/secret owners, and survives the documented restart/upgrade/recovery/uninstall paths. Optional profiles are individually declared. Exact signed bytes tested are exact bytes published after independent evidence review and explicit human publication.

## 7. Programme PR disposition and obligation transfer

Open programme/checklist PRs are not code branches. Keep their history but route work here:

| Historical programme / tracking | Executable owner or gate |
|---|---|
| 97 | This map; coordinator never merges its historical branch |
| 99,102,103,114 | 266,269,332,279,264; delete old entrypoints only after callers cut over |
| 109 | 238 then 333; 277 consumes operational observations |
| 111 | 330; crypto 309; inventory 302; restore consumers 244/245 |
| 112,113 | 316,257,331; profiles 226/227/216 |
| 115 | 307–310; no second daemon secret selector |
| 116 | 235.core then 194/308/332; no extraction/native circular gate |
| 119,120 | 120.native/119.live; exact artifact/process evidence, not another bridge |
| 121,123,124,127,186,190 | 256–264 and 329; old 200/207/209/210 snapshots are source donors only |
| 130 | Optional LSP and actual overlay core/integration findings; no scope promotion |
| 132 | 223/224; 225/236 optional, separate assurance |
| 134,135,136 | 300–305,243–245,134.release; ports first, destructive acceptance last |
| 137,140,185 | 233/234/240/242/215 and final independent review/publication |
| 139 | Exact optional/shipped inventory, per-profile acceptance, default closure |
| 184,189,193 | Measured responsibility cleanup after owner cutovers, not cosmetic new topology |
| 187,191 | 250 delivered; 251 and bounded guard remain; compiler is semantic authority |
| 205,220 | Shared epoch delivered in 258; normative amendment and live boundaries still verified |
| 213,218,221 | 213.schema/280/281; 272; 278/232/279 respectively |

Close a programme only after each preserved obligation is delivered, explicitly superseded by an accepted contract, or explicitly deferred/not-shipped. Never close it merely because a child issue exists. Old source donors remain closed; do not resurrect their architecture or dependency pins.

## 8. Manager and subagent operating packet

Use 5–10 subagents by combining these roles when fewer are needed: (1) scope/contracts/callers; (2) donor/source/license/features; (3) exact identity/migration; (4) native/bounds/cancellation; (5) access/currentness; (6) integration/deletion/reverse consumers; (7) focused fixture/evidence review; (8) independent final review. No new full-project research phase for every task.

Each assignment is one question or one narrow code region. Return: exact source SHA; paths/symbols; primary citations; proposed patch outline; preserved semantics; migration/deletion obligations; blockers. Unknown means unknown, not another self-issued digest. The manager makes decisions and integrates; subagents never resolve Cargo.lock or merge.

One task report records: base/head SHA; owned changed files; dependencies/features; old APIs deleted or legacy read-only with owner; canonical bytes/profile effects; minimum locked check and strict Clippy including immediate reverse consumers; fixtures written and honestly executed/not-run; remaining integration gate. Use existing typed owners and errors rather than adding a new abstraction per concern.

If code already exists, continue or replace it: never implement a second package from stale scaffold text. If an accepted donor cannot satisfy a requirement, document the exact failure and choose only the named fallback from that issue. Two failed approaches require a short causal audit, not repetitive retries. A genuinely missing shared API is assigned to its existing owner before downstream code, not recreated privately.

## 9. Qualification and end-of-project checklist

#215 freezes exact source/artifacts/config/scope/population/profiles/cases/SLOs before measured results. Required baseline lanes: M1 installed product; W1 root/watcher/currentness; P1 deterministic retrieval; P2 complete exact proof; I1 publication/lifecycle; R1 total resources; N1 10,000-query no-write/disclosure. Every shipped document/code/research/adapter profile adds D1/C1/S1/A1 as applicable.

Preserve FAIL/PARTIAL/UNAVAILABLE/NOT_RUN cases. Retain raw cumulative counters/times, exact native process identities, owner inventories, public results/coverage/handles, fault transitions and canary surfaces. Histograms/signatures/status booleans do not replace their subjects. Product repairs create new candidate/run identities; qualification does not modify source or lower the same plan.

Before release: complete current profile inventory; completed migration/removal ledger; no current imports of old FNV/point/client/catalog paths; current normative version/hash and operator guide; reproducible locked/offline build inputs; closed SBOM/licenses and non-Cargo artifacts; standard-user install/provision/restart/upgrade/uninstall; backup and last-copy/key protection; independent verification; final signed subject digest; explicit publication authorization.

## 10. Audit limits and maintenance

This pass reconciles the complete execution programme against inspected current source, delivered PR evidence and detailed existing findings. It is not a new line-by-line proof of the entire repository, donor ecosystem or absence of bugs. Previously reported scope checks are not rerun here. No Rust, Cargo, lockfile, workflow or product feature is changed by this documentation cutover.

The existing technical audit and specialized files retain F01–F154 obligations; a closed planning issue does not resolve a code finding. Maintain the finding-to-owner links and delivery evidence. Update only affected rows after source merges; do not append contradictory schedule comments indefinitely. When an issue body and this map disagree, resolve the concrete interface/phase against higher-level contracts and amend both before writing code. The current manager can continue #256; this full-project map is not a new gate requiring another general audit.
