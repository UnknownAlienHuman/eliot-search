# Analysis registry

This records the refactor audit's inspected surface, findings, changes and remaining work. It is memory for
continuation, not an approval gate, package-status system or substitute for owner instructions.

**Original public source baseline:** `c8e38b6fc9829f1cd38070773b1b3e5b21de18ad`  
**Latest inspected source revision:** `e5350809ba501e88b7c76e32d97cfe0c8838b2ac` (2026-10-23, R012).  
**Historical qualification archive:** `2f9c9720f3e14799a0f8dfefa00077d2fa6a9887/qualification`  
**Update rule:** commit findings and this registry together. Resume from the actual PR head and intervening
changes. Bind execution evidence to SHA/command/targets. Row counts are not an audit completion percentage.

## Status vocabulary

- **CHECKED**: inspected the named static surface for the stated question; not every line or runtime property.
- **PARTIAL**: named source, caller, reconciliation or implementation questions remain.
- **PENDING**: insufficient inspection to direct a change.
- **RUNTIME-ONLY**: requires implementation/execution, not stronger prose.
- **PROTECTED**: unpublished work is not inspected/overwritten; consume its accepted public handoff.

## Coverage ledger

| ID | Area | Exact inspected surface | Status | Finding / change | Remaining work | Confidence |
|---|---|---|---|---|---|---|
| A001 | Documentation authority/consistency | Root authority; bridge/reconcile/preparation/publication/control/registry; R012 query/window/evidence owner docs and old continuation contract | PARTIAL | R012 fixes late retention admission, lazy expiry, raw-versus-represented ranges and request failures treated as item gaps. Window/evidence READMEs now link to their single recipes. | Other current owner documents and actual consumers still need reconciliation. | High named findings, incomplete cross-owner sweep. |
| A002 | Historical governance data | swarm, generated maps, packets/handoffs, context/ticket artifacts | CHECKED | Removed from current tree; Git history is archive. | Code remnants A015/A022. | High disposition. |
| A003 | Workflows/wrappers | .github/workflows, tools/*.ps1, A022 YAML/run | CHECKED | Old validators and temporary automatic A022 removed; one manual Cargo diagnostic remains. | Recheck when workflows change. | High inspected tree. |
| A004 | Workspace inventory | Root manifest and 51 current package manifests; disposition | CHECKED | Each member has a receiving/removal/optional/conditional disposition; xtask removed. Not a target package count. | Actual product-package cutovers. | High inventory. |
| A005 | Entrypoints/product | Daemon/CLI composition; quickstarts; R010 source_sync; R011 preparation_composition; R012 public runtime query/session | CHECKED for named routes | One-shot scan remains the simple runnable path. Source sync still refuses. Inspected paging materializes search before retention admission; current line-session loop has no expiry turn. | Execute connected source/publication/CLI after implementation. | High static, not product execution. |
| A006 | Root/config/control | Earlier root/config/journal/snapshot inventory; R011 control recipe and pinned redb ReadTransaction source | PARTIAL | Typed state selected. R011 supplies missing restartable full-build input capture using pending/target tables; process-held read snapshot is not durable selection. | Implement capture/application transactions and platform behavior; no runtime recovery proof. | High source/document distinction. |
| A007 | Source acquisition/registry/reconciliation | Admission/source stack; R010 comparator/currentness/catalog/sync; R011 registry recipe | PARTIAL | R010 source defects remain unimplemented. R011 strengthens proposed current-row application against ABA/re-registration, without putting concurrency revisions into transform identity. | Real source-owner/sync cutover and accepted #598 handoff; execute named cases. | High inspected branches, disconnected product. |
| A008 | Private storage/key work | Public #598 contract only | PROTECTED | Old six-domain/purge/compatibility programme is not inherited. | Accepted public handoff; do not inspect/reset/duplicate private work. | Boundary known, implementation unknown. |
| A009 | Preparation/identity | Prior unitizer/identity audit; R011 materializer profile, request/budget, product pipeline/read/model and old FUNCTIONS | PARTIAL | Same golden/limit identity coupling as lexical; separate request/execution budgets can read before the tighter limit. Replacement separates semantic reuse, guarded installation and retry eligibility. R012 checks the receiving evidence coordinate contract separately. | Implement current preparation; verify config-only retry, budget preflight, ABA, source-bound artifacts and stale failure handling. | High source findings; target scenarios unrun. |
| A010 | Qdrant/donors | Bridge callers and Qdrant 1.19 mutation/readback/BM25/tokenizer/IDF; real/connect/gate | CHECKED for named static questions | Gate token removed at 7d92fb8b; actual runtime integration, auth/build/deadline/ledger replacement remain. | Connect real path; late-write/relevance/scale checks. | High named source, no product claim. |
| A011 | Publication/view/cleanup | Prior machine/lifetime audit; R011 publication/control recipes; R012 window lifetime boundary | PARTIAL | Two-sided desired/published comparison and captured build inputs specified. R012 distinguishes expired-window removal from active work still holding its guard. | Implement publication/capture/cleanup and actual cancellation/release; timestamps alone do not establish reader drain. | High counterexamples; no executed crash proof. |
| A012 | Query/evidence/exact | Prior query audit; R012 shared validator through validate, daemon paging/catalog/prepared output and direct exact helper | PARTIAL | Port errors collapse to item gaps; raw-range length/exact-map contract cannot represent transformed excerpts unchanged. Retention is count-only and after full search. Replacement separates coordinate domains, failure scope, bounded nomination retention and coverage meaning. | Implement real query/reader/page consumers; preserve partial-result correctness and test mapped evidence/cancellation. Not every caller/feature inspected. | High named source; target behavior unrun. |
| A013 | Protocol/client/CLI | Prior provider/daemon/CLI; R012 line-session/read_line and page commands | PARTIAL | Session window policy lacked an idle-expiry mechanism. Existing synchronous loop does not itself service expiry while waiting for input. Target uses existing event/control path, not a cleanup service. | Real timed control loop, active-work cancellation/guard release, same-session pages and expansion. | High inspected path; not all transports. |
| A014 | Product depth | Orientation/comparison/document requirements and old enrichers/resolvers/workers/overlay | PARTIAL | Observable goals retained; scaffolds are not capabilities. R012 does not add a document extractor. | Concrete producer/consumer selection and quality evaluation. | Medium. |
| A015 | Tests/governance code | xtask/workspace/lock/workflows; inspected ownership/source-text tests | PARTIAL | xtask, ledger and exclusive dependencies removed; 11 Qdrant, 5 evaluation, 33 daemon and 17 preparation/store structural tests removed, behavior suites retained. R012 confirms spine completeness guard is useful, not a deletion target based on its name. | Residual structural consumers. No test deletion or compiler run in R010-R012. | High removed/named surface; residual search incomplete. |
| A016 | Donors | Prior redb/Qdrant/matching/Git/ignore; R011 redb 2.6.3; R012 official Vec truncate documentation | PARTIAL | Vec truncation does not reduce allocated capacity. This stable contract does not establish measured allocator RSS or target memory use. | Concrete donor questions only; no dependency/toolchain upgrade or whole-donor audit. | High named mechanisms. |
| A017 | Qualification tree | Complete tree inventory, representative reports, consumer searches | CHECKED | Archived at 2f9c9720; policy/current blockers only. | Runtime approval remnants A022. | High disposition. |
| A018 | Auxiliary filenames | Package/binary roots, standard/nonstandard helper files | CHECKED for filename/disposition scope | Wave/profile/DIRECT/duplicate helper specs removed; distinct DIAGNOSTICS retained. | Surviving semantics are A001. | High stated scope. |
| A019 | Issues/PR handoff | Current chain; #352/#214/#525; #263 R009; #271/#239 R010; #270/#331/#261 R011; #275/#293 R012 | PARTIAL | Current issues linked to owner recipes rather than stale pinned algorithms; no whole-queue reread. #525 complete, #214 open. R012 reconciles window/evidence failure and coordinate rules. | Refresh other affected issues with real owner changes. | High named issues only. |
| A020 | Runtime evidence | Windows, Qdrant late writers/IDF, relevance/scale, reader growth, restore, restart; R012 idle windows and mapped-page cancellation | RUNTIME-ONLY | Static counterexamples are not failure reproductions or product readiness. | Implement and execute with exact SHA/commands/raw outcome. | Not statically established. |
| A021 | Compiler baseline | Ubuntu Rust 1.98 runs 39867915623, 39868025680, 39868205817, 39868374272, 39869930525, 39870086718, 39872058276 | PARTIAL | Latest verified A022 run covers search-qdrant-bridge --lib at 7d92fb8b only. Earlier daemon check fails. R009-R012 make no new compiler claim. | Consumers, test compilation, Windows and daemon outside that result. | High recorded scope; no whole-product build. |
| A022 | Runtime qualification | Bridge real/connect/gate; lexical code_identifiers/encoding/fixtures | PARTIAL | Qdrant gate removed/library-checked. Lexical fixture-backed activation remains. R011 finds analogous profile-identity coupling in materializer (A009). | Remove remaining runtime ceremony with callers; keep useful causal examples as tests. | High named findings, no exhaustive caller proof. |

## Pass log

### R001 — authority/governance removal

Removed Architecture 8.4 master, wave/handoff/status/swarm/generated authority and redundant workflow/tool
surfaces through `a4af41f8`. Historical evidence remains in Git.

### R002 — first package-local wave cleanup

Inspected/removed CLI/provider W8, query W7, query-window helpers, Qdrant bridge P18 and control
journal/cutover/intent/quarantine/checkpoint copies. They prescribed retired mechanisms or repeated current
owner rules. Commit `2f9c9720`.

### R003 — qualification archive disposition

Inspected complete tree, representative reports and consumers. Retained policy/current blockers; exact
archive `2f9c9720`. Commit `49f5c46b`.

### R004 — standard supplements and GC contradiction

Inspected old xtask supplement policy and remaining standard supplements. Daemon W8/W10 ordered grants,
receipts, capability snapshots, qualification, quarantine/migration; others duplicated owners.
`search-index-reclaimer/FUNCTIONS.md` still required online local GC while implementation chose stopped-root
maintenance. Corrected ownership and removed standard supplements in `140852e4`.

### R005 — nonstandard package-local closure

Inspected current package/binary roots, not default-branch search. Checked journal/checkpoint/quarantine/
recovery/profile/integration/scale/security/snapshot names. Removed control SECURITY_RESTRICTION_JOURNAL;
moved replacement identity/collision/occurrence rules from S11_PROFILE into point-identity FUNCTIONS, which
had still frozen old S11. README reduced. DIAGNOSTICS retained as a distinct read-only operation.

### R006 — xtask/digest-governance removal

Inspected manifest/lock/aliases/workflows and all xtask command/module families; each served retired governance.
`f9275731` removed xtask, 3.57 MB ledger, workspace member and cargo_metadata; `9d2fe65a` regenerated lockfile.
Run `39868374272` checked removal/locked metadata/search-contracts and strict Clippy under Rust 1.98.
Temporary verifier removed. Broad daemon check failed on old composition; A021 retains the limitation.

### R007 — source-shape tests

Removed inspected ownership tests: 11 Qdrant (`07c90c14`), 5 evaluation (`30d1921f`), 33 daemon plus four DIRECT
notes (`789d4fb4`), 17 materializer/unitizer/revision-store (`a0de1610`). Retained CRUD/readback/recovery/IDF/
metrics/materialization suites. Run `39869930525` passed locked check/strict Clippy for search-materializer,
search-unitizer, search-revision-store, search-qdrant-bridge and search-eval.

### R008 — stale references and runtime qualification

Run `39870086718` searched active Markdown/Rust/Cargo. Corrected xtask/package-status/qualification references;
removed duplicate TOML_PROFILE_2; distilled ADR 0005/Research adapter. At that checkpoint Qdrant required
QualifiedGate and lexical issued fixture-backed receipts. Later edits existed by `7d92fb8b` while the registry
lagged after interruption. R009 corrected the statement rather than repeating the code cut.

### R009 — interrupted-cut recovery and qualification audit (2026-10-23)

At `7d92fb8b5532b5928bdb90ddd42d5685e5f01d48` read bridge real.rs, real/connect_schema/connect.rs,
qualified/gate.rs and README/AGENTS/FUNCTIONS; lexical code_identifiers.rs constructor/preflight/profile_definition,
code_identifiers/{encoding,fixtures}.rs and owner notes; root instructions, workflows and exact Actions log.

Gate removal existed; connect takes no token and report validation returns (). Auth/build/timeouts/ledger
remain. Fixed contradictory bridge README/AGENTS mutation/empty-scope rules. Lexical hashes fixtures/limits
into profile identity and runs fixture admission under operation budgets; 16-byte counterexample documented
in lexical FUNCTIONS. No lexical execution/change. Default-branch caller search was not exhaustive proof.

Verified historical run [39872058276](https://github.com/UnknownAlienHuman/eliot-search/actions/runs/39872058276),
job 120148813047, 2026-10-22, Ubuntu 24.04, Rust 1.98.0, checkout `7d92fb8b5532b5928bdb90ddd42d5685e5f01d48`:

```text
cargo +1.98.0 check -p search-qdrant-bridge --lib --locked
cargo +1.98.0 clippy -p search-qdrant-bridge --lib --locked -- -D warnings
```

Both passed; no tests, daemon, reclaimer, Windows, live Qdrant or recovery ran. Commit `c0fcf294` changed docs
and removed the leftover automatic verifier. Do not add push triggers to compensate for missing manual dispatch.

### R010 — source observation, selection and freshness (2026-10-23)

At `c0fcf2947b0ce6118ae67ee94e557632bd2a809d` read:

- source-reconcile/src/lib.rs through the unit-test entry;
- source-records/src/root_currentness.rs through line 570;
- daemon source_roots/kernel/catalog.rs through line 300 and complete owner_composition/kernel/source_sync.rs;
- source/reconcile/admission/identity owner docs, old reconcile FUNCTIONS at c8e38b6, #271/#239;
- ignore walk.rs at 4649aa9700619f94cf9c66876e9549d83420e16c and published 0.4.33 entry/walker docs.

Found Missing bypassing root check, first physical-key binding dropping content change, physical/logical
identity mismatch, erased causes, hints leaving workspace_current true, root probes clearing content hints
and root-slot changes not rebasing queued positions. Replacement also missed filtered omission versus absence,
attached-entry ignore errors, depth-zero filtering bypass and parser-changing rename. Fixed reconcile recipe,
README/AGENTS and #271/#239 in `7c7fd36c`; A007 returned to PARTIAL.

Static counterexamples only. Sync remains unavailable. Default-main reconcile_source search found its definition,
not exhaustive absence of branch/feature callers. No Rust/Cargo/tests/build/private #598 changes. Local clone
failed DNS; exact files were read through GitHub. No notify/gix upgrade or whole-donor review.

### R011 — preparation identity and publication convergence (2026-10-23)

Inspected exact head `7c7fd36cfc2e076458de141f0a4c037ade580f1c`:

- `crates/search-prep/search-materializer/src/profile.rs`, `profile/{change,digest}.rs`;
- `crates/search-prep/search-materializer/src/request.rs`, `request/{budget,validate}.rs`;
- `crates/search-prep/search-materializer/src/product/{pipeline,read}.rs` in full, `product/model.rs` lines 1-210;
- complete `bins/eliot-searchd/src/preparation_composition.rs`;
- materializer AGENTS/FUNCTIONS, publication/control/registry/access FUNCTIONS and current #270/#331/#261;
- old materializer FUNCTIONS at c8e38b6, lines 1-190, including golden/bounds profile and acquisition requirements;
- redb `Cargo.toml` lines 1-45 and `src/transactions.rs` lines 2320-2520 at
  `f4e3eb69dc2a4b4e01fefff0b9e039c6c7ab98f6` (Cargo confirms 2.6.3); std Read documentation for byte-cap semantics.

Source findings:

- Materializer profile_digest includes golden and all resource ceilings; classify_profile_change turns a new
  identity into reprepare/reproject or rejects an unchanged revision. This repeats lexical's test/policy coupling.
- Request validation enforces its own budget, but execution has an independent public context; pipeline does
  not recheck the execution input limit before calling the reader. Read port cannot observe that context.
- Zero-length rejection remains in request validation and retained reader: exact locations of an already-known
  unfinished empty-input correction, not a newly implemented feature.

Replacement-design findings and owning corrections:

- [Preparation](../../crates/search-prep/search-materializer/FUNCTIONS.md): separate output-affecting semantics,
  current installation guard and retry conditions. Fail-only limit changes preserve valid successes but can
  reactivate prior failures; output-changing limits remain semantic. Preflight the actual execution input limit.
- [Source state](../../crates/search-source/search-source-registry/FUNCTIONS.md): content/membership equality
  does not guard ABA/remove-readd. Use non-reset current-row/registration guards without putting them into
  transform identity. Keep source-bound metadata when reusing pure content.
- [Publication](../../crates/search-index-qdrant/search-publication/FUNCTIONS.md): compare desired and published
  inventories, including published-only deletions and explicit gaps, not only successful prepared artifacts.
- [Control](../../crates/search-control-redb/FUNCTIONS.md#restartable-generation-input-capture): capture chosen
  full-build input refs under one read snapshot into existing unserved target tables before external effects;
  partial capture restarts, completed capture resumes exact inputs. A cursor against new current rows is not
  the old snapshot. Capture is O(selected metadata), not constant work or another snapshot platform.

Evidence boundary: static code/document counterexamples; no application failure reproduced, no Rust/Cargo,
new dependencies, tests, Clippy, CI or private #598 work changed/executed. Clone failed DNS; source access and
publication use the GitHub connector. Two versioned redb documentation pages were unavailable, so the pinned
GitHub implementation was read rather than substituting newer API docs. The std page was rendered as 1.99.1;
only the long-established Read/take contract was used, not a toolchain upgrade or a new 1.98 execution claim.

A009/A011 remain PARTIAL. These changes do not close the actual preparation/current-row/publication integration,
remaining code consumers, full current-document sweep, relevance/scale or platform/restart evidence. Prior
successful library checks are not evidence for these proposed state transitions.

### R012 — window admission, idle expiry and evidence failure domains (2026-10-23)

Inspected at `e5350809ba501e88b7c76e32d97cfe0c8838b2ac`:

- `bins/eliot-searchd/src/continuation/kernel/{catalog,spec}.rs` and `catalog/prepared.rs` in full;
- `bins/eliot-searchd/src/public_runtime_service/kernel/{query,session}.rs` in full, `src/protocol_io.rs` lines 1-145;
- `bins/eliot-searchd/src/direct_store/model.rs` and `src/direct_preparation/spine.rs` in full;
- `bins/eliot-searchd/src/query_composition/service.rs` in full (separate canonical registry/grant host, not proof of paging integration);
- `crates/search-query/search-candidate-validator/src/lib.rs` lines 1-555, through validation/error mapping;
- root architecture/implementation; query overview; window/evidence README/AGENTS/FUNCTIONS; projector FUNCTIONS;
- old continuation FUNCTIONS at c8e38b6 and current #275/#293. Directory trees located these files; unrelated files were not thereby reviewed.

Findings and changes:

- Daemon search builds the result before catalog retention admission. Catalog counts matches, not their owned
  capacity/strings; truncating the vector retains its capacity. Existing upstream DIRECT corpus bounds were
  checked, so no claim of infinite allocation is made. Window recipe now reserves before retrieval and accounts
  for retained and temporary allocations; it freezes only a finite nomination set, not a completeness claim.
- Expiry is access-driven in the inspected catalog. The line loop has no independent expiry turn. New view
  guards must not inherit this lifecycle: existing control/event loop expires idle records and cancels active
  work, but cleanup still respects any active guard not yet released. No new timer/cleanup service is selected.
- Shared readback maps every port error to SourceUnreadable and then a candidate Gap. The prior README/#293
  also described request exhaustion as a gap. Evidence/window recipes now separate request stop from item loss
  so cancellation cannot consume unprocessed candidates or trigger endless refill.
- Raw span length/exact-map checks cannot represent transformed excerpts unchanged. UTF-16LE 41 00 -> UTF-8 41
  demonstrates the domain mismatch. Existing preparation maps provide represented ranges and truthful source
  location; no new coordinate platform and no false exact-byte claim.
- Confirmed a useful old behavior: verify_spine_gate rejects contradictory completeness claims but accepts
  honest partial results; literal validation checks byte/character boundaries. Neither is a deletion target
  merely due to its name. Prepared-page output-before-cursor-commit is also retained.

Rewrote the two owner recipes, reduced their READMEs to entrypoints and reconciled #275/#293. A012/A013 are
PARTIAL, not blanket checked subsystems. No exhaustive current-branch caller or whole-codebase proof.

Donor check: the versioned Rust 1.98 Vec page was inaccessible. Official
[stable Vec documentation](https://dev-doc.rust-lang.org/stable/alloc/vec/struct.Vec.html#method.truncate)
confirms truncate leaves capacity unchanged. Only that longstanding contract is used; no dependency upgrade,
Rust 1.98 execution, heap/RSS measurement or new native/Qdrant claim follows from it.

Evidence boundary: static source and design counterexamples only. R012 changes Markdown/issues, not Rust,
Cargo, workflows, tests or private #598 code. No new Clippy/build/runtime result. A local clone failed DNS;
reads/writes used the GitHub connector. Remaining implementation and focused cases are in the two owner recipes.
