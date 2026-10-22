# Static refactor audit closure and rewrite map

**Inspected public source baseline:** `c8e38b6fc9829f1cd38070773b1b3e5b21de18ad`  
**Audit/refactor documentation:** PR #610 through the final documentation sweep  
**Status:** sufficient input for rewriting the current-format documentation and implementation; not a line-by-line source proof, build result, runtime qualification or product-readiness certificate.

The earlier blanket “everything is checked” claims were wrong. This record closes only the static architecture/documentation audit: the major owners, real callers, competing applications, historical requirements, selected donor semantics, active implementation issues and documentation authority have been reconciled enough to begin the rewrite without another audit gate.

## One authority chain

Implementation work uses only:

1. [`AGENTS.md`](../../AGENTS.md) — repository method and current coordination rule;
2. [`docs/architecture/README.md`](../architecture/README.md) — product purpose, observable requirements and principles;
3. [`docs/implementation/README.md`](../implementation/README.md) — selected current-format mechanisms and order;
4. the nearest owner `FUNCTIONS.md` — one concrete operation recipe and source seams;
5. the active issue — scope, consumers and completion evidence for that change.

Audit and donor documents explain evidence and limitations. They do not authorize code, freeze APIs or add gates. Pseudocode names, package rows, donor method catalogues and issue nouns are not orders to create types, traits, services, crates, registries or workers.

The 8.4 master, package-status graph, P00 contracts, handoff packets and `swarm/**` control plane were removed from the current tree in this sweep. Git history remains the archive. They cannot be used as a second specification.

## Coverage completed for the rewrite

| Area | What was checked | Rewrite consequence |
|---|---|---|
| Workspace and entrypoints | 52 Cargo members, default/features, daemon/CLI applications, principal production callers and fixture-only paths | One CLI/daemon application; every member has KEEP/MERGE/DELETE/OPTIONAL/CONDITIONAL disposition, not a target package count |
| Root/config/control | root/owner composition, configuration registry, generic redb journal/snapshot and registry replay | One real root lock, typed settings/current rows and finite unfinished external work; no eternal operation archive |
| Source and retained objects | admission/receipt/provenance/history/registry families, safe read, revision stores and public #598 contract | One stable acquisition, one immutable writer/reader and one current source head; protect private #598 overlap without inheriting its obsolete programme |
| Preparation and identity | materializer/unitizer paths, repeated verification/copies, UnitId/S11 rewrite amplification and empty/gap cases | One materialization, structure-first units, conservative occurrence reuse, stable retrieval identity and versioned evidence |
| Qdrant and lexical | real SDK adapter/callers, mutation/readback semantics, native BM25 tokenizer/inference, filtered IDF source and late-write behavior | One private real runtime; immutable published retrieval state; exact-name plane separate from BM25; no unmeasured O(top-k) or vector-readback claim |
| Publication/lifetime/cleanup | three publication machines, pending recovery, old-view snapshots, pins/reclaimer/retention and restore code | One pending publication/current view/unfinished cleanup owner; old readers and unresolved writers both constrain deletion; stopped-root local GC |
| Query/evidence/session | recipes/capabilities/plans/access proofs/executor/validator/projector/handles/continuations and exact matcher | Direct Search/Exact/Continue/Expand, one request context, retained evidence, one live query window and explicit exact input semantics |
| Protocol/client/CLI | pairing/grant/manual JSON, parallel line protocol, typed and legacy clients, output/cancellation behavior | One bounded framed current protocol; same-session window lifetime; actual cancellation/partial-output limits; no local grant framework |
| Product depth | old orientation/comparison/document/overlay/model requirements and their disconnected scaffolds | Preserve source-backed navigation, ambiguity, comparison and document goals; add only concrete producers/consumers, not empty frameworks |
| Tests/tooling/governance | source-shape tests, generated package maps, digest ledger, xtask analyzers, P00/swarm/qualification authority | Keep causal behavior checks; delete structure/governance machinery with consumers; minimal compiler/Clippy after real code |
| Documentation and issues | all 288 PR paths, current root/package instructions, current open-issue queue and obvious duplicate/superseded tasks | One owner rule per behavior; #352 is the concise current programme; obsolete harness/graph/lint/status tasks are closed rather than implemented |

The detailed evidence is in [the distilled audit](ARCHITECTURAL_AND_CODE_AUDIT_2026-10-17.md), [workspace disposition](WORKSPACE_MEMBER_DISPOSITION_2026-10-21.md) and [donor evidence](DONOR_VERIFICATION_REGISTER_2026-10-09.md).

## Errors found in the replacement itself

The audit repeatedly created new failure modes while deleting the old architecture. These corrections are mandatory rewrite inputs:

- a queue notification is not durable work; current input/outcome comparison recovers lost wake-ups;
- stale preparation cannot overwrite a newer source/profile/removal;
- successful empty input is not extraction failure or unavailable index;
- a copied root is an archive/import, not an authorized restart of original Qdrant pending/cleanup;
- readable copied ciphertext is not writable nonce-counter continuity;
- a redb snapshot does not snapshot mutable Qdrant vectors/filter payload;
- observed state does not prove all old Qdrant writers are terminal;
- native-generated BM25 vectors have no locally known coefficient oracle;
- scoped IDF can enumerate a broad population even when `top_k` is small;
- Qdrant word tokenization does not preserve punctuated identifier identity;
- the existing chunk matcher is not a compile-once cancellable retained-source scanner;
- a one-shot child cannot return a continuation locator whose process is immediately destroyed;
- a blocked write is not cancelled by checking a clock before the call;
- local flush is not peer acknowledgement or exactly-once delivery;
- a bounded page does not bound work performed before page selection;
- audit/issue pseudocode and vendor API catalogues must not become implementation checklists.

If a later design changes one of these decisions, it must provide a concrete counterexample and owner, not another general framework.

## Rewrite rules against neural slop

1. **Observable behavior first.** State input, output, failure, cost and owner before naming a new abstraction.
2. **One fact, one owner.** Source bytes, current control, retrieval state, evidence and session state must not be duplicated across databases or packages.
3. **One algorithm, one document.** Architecture states why; implementation selects the mechanism; owner notes specify the operation. Issues link instead of copying another algorithm.
4. **No proof-shaped substitutes.** Digests, receipts, booleans, capability names, qualification records and caller-supplied readbacks do not establish unperformed effects or unread bytes.
5. **No catalogue completion.** Implement only SDK/donor operations with real connected consumers.
6. **No hidden cost claims.** `bounded`, `streaming`, `native`, `incremental`, `snapshot` and `top-k` require the actual preceding work and retained memory/disk cost to be stated.
7. **No compatibility for pre-release internals.** Protect user data; reject unsupported roots before writes. Do not preserve dead APIs, receipts, bytes or fixtures through adapters.
8. **No scaffold as capability.** A package, worker, schema, handler name or fixture without a real producer/consumer is unavailable and may be deleted.
9. **Delete with cutover.** Move useful behavior, change actual callers, then remove the displaced path, tests, docs, dependencies and generated governance.
10. **Claims follow execution.** Compilation is not Windows durability, live-Qdrant recovery, relevance, scale or product readiness.

## Current implementation programme

Issue #352 owns only coordination. The implementation chain is:

```text
#330 + #270 + #331
  retained bytes → current source head → recoverable preparation

#259 + #329
  changed-source projection → stable retrieval point + versioned evidence

#263 + #261 + #300 + #303
  real Qdrant → atomic current view → reader/writer-safe cleanup

#272 + #291 + #293 + #279
  selected scope → one request context → retained evidence → Search

#275 + #280 + #332
  live query window → one framed protocol → actual CLI

#249
  explicit retained-source exact scan
```

Optional source-backed capabilities follow real producers: #281 orientation/comparison, #227 PDF, #335 Git views, #336 editor overlay, #219 MCP leaf and #334 managed Qdrant child. Deleting their current scaffolds does not declare their user requirements complete.

#598 remains protected unpublished storage work. Only its accepted public handoff may be consumed; its six-domain/purge/compatibility programme is not a repository-wide prerequisite.

## Remaining evidence — not a reason for another audit wave

The following are implementation and measurement work, not missing static documentation authority:

- compiler/Clippy after each actual cutover and deletion;
- Windows no-clobber/namespace durability and blocked-I/O cancellation;
- live Qdrant unknown-write/late-write behavior, scoped-IDF cost and relevance;
- old redb reader growth, cleanup backlog and stopped-root GC disk/time;
- protected-storage handoff and supported restore/import mode;
- real large-repository/navigation/comparison/document quality;
- end-to-end source → restart → Qdrant → verified excerpt.

A new audit/status/qualification control plane is not required before implementation. New findings belong in the owning issue/document and must change the concrete design rather than create another parallel report.
