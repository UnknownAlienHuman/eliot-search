# ELIOT Search — workspace member disposition

**Source workspace:** public `main` at `c8e38b6fc9829f1cd38070773b1b3e5b21de18ad`  
**Purpose:** give every one of the 52 Cargo workspace members a concrete refactor disposition.

This is a target map, not a claim that the Rust changes have landed. `KEEP / REWRITE` preserves an effect or
algorithm owner, not its current API or size. `MERGE / INLINE` means move the useful behavior to the real
owner and remove the package. `DELETE` means no baseline replacement package. `OPTIONAL` means the current
implementation does not block the product and may be replaced only for a selected real consumer. `#598`
means final disposition depends on that active private handoff.

## Summary

| Disposition | Count |
|---|---:|
| KEEP / REWRITE | 17 |
| MERGE / INLINE, then delete package | 15 |
| DELETE current package/path | 13 |
| OPTIONAL selected profile after baseline | 5 |
| CONDITIONAL on #598 handoff | 2 |
| **Total** | **52** |

Package count is not an architectural invariant. The target workspace should contain only owners which still
isolate a substantial dependency/effect or a coherent algorithm after callers move.

## Contracts, domain and control

| Member | Disposition | Useful responsibility retained | Current mechanisms removed / moved | Primary implementation owner |
|---|---|---|---|---|
| `search-contracts` | KEEP / REWRITE | Small immutable IDs, locations, selected scope, current Search/Exact/Continue/Expand DTOs, evidence cards/gaps and stable retrieval/view semantics shared by real owners | Eleven recipes, plans, grants, capabilities, proof/lifecycle families, frozen Qdrant schema and generic canonical/digest framework | #280, #259 |
| `search-domain` | MERGE / INLINE | Pure rules only where they remain nontrivial | Global transition/fingerprint catalogue; rules move to source, publication, query and evidence owners | #352 / each receiving issue |
| `search-ports` | DELETE | None as a global package | 23 global traits, 80 operations and conformance fakes; a tiny consumer-owned trait may exist only for multiple real implementations | #214 / receiving owners |
| `search-config` | KEEP / REWRITE | One typed `DaemonSettings`, bounded TOML/Serde decode and selected overrides | Custom parser/tree/registry/fingerprint/diff/action/receipt platform | #333 |
| `search-control-redb` | KEEP / REWRITE | Short typed transactions and point/bounded-page reads for current source/prepared/view/pending/cleanup rows | Generic operation journal, full snapshots, registry replay, migration/quarantine/receipt histories | #270, #261, #333 |

## Runtime, root and storage

| Member | Disposition | Useful responsibility retained | Current mechanisms removed / moved | Primary implementation owner |
|---|---|---|---|---|
| `search-runtime-owner` | KEEP / REWRITE | Current-format root inspection/init and one process-lifetime OS guard | Owner epochs, succession, dual/sealed/migration ownership ceremony and receipts | #266, #360 |
| `search-os-secrets` | DELETE | No generic secret platform | Catalogue, lease, purpose, rotation, revocation and backend-neutral lifecycle; selected secrets remain with concrete owner | #280, #263, #598 |
| `search-revision-crypto` | CONDITIONAL #598 | At most the selected pure AEAD primitive directly used by accepted storage writer | Standalone unused profile/package if #598 does not adopt it; no generic key lifecycle | #598 / #330 |
| `search-os-secrets-windows` | CONDITIONAL #598 | Only concrete Windows effects used by the accepted encrypted storage profile | Provider pairing, Qdrant refs, generic DEK candidate/current lifecycle, migration work meters and duplicate wrappers | #598 / #330 |
| `search-provider-transport-windows` | DELETE | None for baseline child stdio | Client-only old named-pipe adapter tied to pairing/grants; a future persistent pipe is a new current-protocol implementation | #332 |
| `search-retention` | DELETE | None as a generic lifecycle package | Leases, graph sweep, purge, tombstones, restore quarantine, legal holds and secure-erasure evidence | #302, #303 |

## Source ownership

| Member | Disposition | Useful responsibility retained | Current mechanisms removed / moved | Primary implementation owner |
|---|---|---|---|---|
| `search-source-admission` | MERGE / INLINE | Small pre-read policy decision and post-read classification actually used by ingestion | Universal policy/proof platform and duplicated currentness | #270 |
| `search-source-admission-receipt` | DELETE | None | Permanent admission receipt/archive; actual source-head/object state is authority | #270 |
| `search-source-provenance` | MERGE / INLINE | Minimal source kind/locator/profile fields needed in `SourceRecord` or prepared manifest | Separate provenance archive and histories | #270, #331 |
| `search-source-records` | MERGE / INLINE | Current root/source/revision row types | Forwarding package and legacy codecs | #270 |
| `search-source-history` | MERGE / INLINE | Current occurrence/incarnation/head semantics where required | General staged/replayed history and operation envelopes | #270, #259 |
| `search-source-registry` | MERGE / INLINE | Bounded current root/source lookup and selected-source snapshot | Full in-memory registry restore, portfolios/history, generic namespace cutover and batch replay | #270, #271, #272 |
| `search-source-identity` | MERGE / INLINE | Stable source identity rules used by the source owner | Separate legacy digest/profile/migration package; fixed 20-byte Git assumption | #270, #335 |
| `search-source-reconcile` | MERGE / INLINE | One bounded dirty queue, inventory and conservative retirement | In-memory operation ledger and disconnected currentness abstractions | #271 |
| `search-safe-reader` | KEEP / REWRITE | Concrete stable final-handle/file/Git read under admitted root/view | Diagnostic parent-as-root adapters and caller-fabricated security/location state | #267, #335 |
| `search-revision-store` | KEEP / REWRITE | One immutable object writer/reader for source, prepared and operation artifacts | Duplicate legacy/materializer stores and migration owner | #330 |

## Preparation and retrieval inputs

| Member | Disposition | Useful responsibility retained | Current mechanisms removed / moved | Primary implementation owner |
|---|---|---|---|---|
| `search-materializer` | KEEP / REWRITE | One materialization from retained bytes into mapped text/document representation | Reparse/reopen verification between adjacent stages and duplicate artifact writer | #331 |
| `search-unitizer` | KEEP / REWRITE | Structure-first units, local split/merge and exact source coordinate maps | Revision/absolute-offset/ordinal identity, unnecessary canonical serialization/copies | #331, #259 |
| `search-code-enricher` | OPTIONAL | Selected outline/semantic fact importer only after a real consumer/corpus exists | Current custom line/brace Rust parser and universal code-intelligence claims | #281 or later selected issue |
| `search-lexical` | MERGE / INLINE | Bounded code-identifier/path/title/heading expansion text when measured useful | Local BM25/sparse vector/IDF engine and duplicate retrieval profile owner | #331, #263 |

## Qdrant, identity and publication

| Member | Disposition | Useful responsibility retained | Current mechanisms removed / moved | Primary implementation owner |
|---|---|---|---|---|
| `search-projection-planner` | KEEP / REWRITE | Ordered bounded publication/manifest pages carrying uploadable text/vectors and view bindings | Whole-generation object graph, complete manifest copies and digest-plan bureaucracy | #259 |
| `search-point-identity` | KEEP / REWRITE | Closed stable point preimage, full identity digest, 128-bit address and collision readback | Legacy root identity registry and revision/representation/Unit-bound S11 v1 | #329, #259 |
| `search-qdrant-supervisor` | DELETE current package | None for configured-endpoint baseline | Speculative child authority/Job/ACL/readiness platform with no product caller; optional managed child may be rebuilt later | #334 optional |
| `search-qdrant-bridge` | KEEP / REWRITE | One private SDK client/runtime, exact schema/point/vector/payload mutations/query/readback and structured vendor errors | Runtime qualification credential, API-key equality lease, process-local replay ledger, renewed deadlines and duplicated citation/digest state | #263 |
| `search-publication` | KEEP / REWRITE | One pending operation, bounded apply/readback cursor, committed current view and cleanup trigger | Daemon duplicate publisher, phase/compensation/proof systems and fake commit observations | #261 |
| `search-epoch-pins` | DELETE | None as a public pin package | Route/epoch owner IDs, renewal and sampled watermarks; replaced by current-view `Arc` guards | #300 |
| `search-index-reclaimer` | DELETE | Exact delete/readback helper moves to publication/bridge if useful | Reclaimer plans/proofs/checkpoints/journal and separate reclamation authority | #303 |

## Query and evidence

| Member | Disposition | Useful responsibility retained | Current mechanisms removed / moved | Primary implementation owner |
|---|---|---|---|---|
| `search-access` | MERGE / INLINE | Non-widening `ResolvedScope` from pinned view and optional real restricted session intersection | Grants, snapshots, per-member plans, overlap proofs, security generations and mutation barrier | #272, #274 |
| `search-overlay` | OPTIONAL | At most one thin process-local editor branch shadowing matching source occurrences | Current disconnected overlay operating system, persistence/security/checkpoint machinery | #336 |
| `search-exact` | KEEP / REWRITE | Direct Aho-Corasick literal scan and optional bounded regex adapter over one pinned inventory | Compile/execute proof plane, permits, supplied readbacks, denominator/checkpoint system and duplicate matcher | #248, #249 |
| `search-subject-resolver` | OPTIONAL | Selected bounded source-backed orientation helper after real producers exist | Generic subject-proof/ambiguity authority and disconnected optional depth | #281 |
| `search-query-planner` | DELETE | At most tiny route/profile grouping helpers inline in query owner | Public plans, capability sets, recipes, grants, dependencies, fingerprints and relative leg budgets | #279 |
| `search-retrieval-executor` | DELETE | Direct request context/backend call moves to query owner; optional tiny rank merge remains private | Scheduler snapshots, tickets, terminal maps, relative deadlines, count-exact completeness and default fusion OS | #291, #278 |
| `search-candidate-validator` | DELETE | Point→manifest→retained evidence lookup inline in query/source owner | Caller nomination/readback proof, echoed digest/assurance authority and separate validation lifecycle | #293 |
| `search-comparator` | OPTIONAL | Concrete source-backed comparison after real relation/provenance producers exist | Generic observation/proof comparator and name/digest-based contradiction claims | #281 |
| `search-handles` | DELETE | Exact range-read helper moves to evidence/window owner | Second handle database, random tokens, grants, invalidation and arbitrary full-revision expansion | #275 |
| `search-result-projector` | MERGE / INLINE | Preserve final order and format bounded cards/gap summary | Raw-score resort, mandatory handles, caller coverage/proof input | #293, #279 |
| `search-continuation` | DELETE | Prepared-page commit-after-output behavior moves to one `QueryWindow` | Durable replan, issued fingerprints, token/security histories and second continuation database | #275 |

## Evaluation, protocol and adapters

| Member | Disposition | Useful responsibility retained | Current mechanisms removed / moved | Primary implementation owner |
|---|---|---|---|---|
| `search-eval` | OPTIONAL | Small post-product qrels/run/metric tool only when retrieval evaluation is actually run | Runtime/release authority, receipt/proof/evidence platform and mandatory gates | Later evaluation issue |
| `search-provider-protocol` | KEEP / REWRITE | `u32-le` bounded framing, current Serde DTO envelope, correlation/cancel and one terminal response | Pairing/MAC/binding/grant/manual JSON, V1 recipes/V2 parallel wire and replay authority | #280 |
| `search-provider-client` | KEEP / REWRITE | Small synchronous child-stdio/local framed client with one deadline and bounded I/O | Native expectations, pairing/profile/grant/sequence history and legacy-loopback | #280, #332 |
| `search-eliot-adapter` | DELETE current package | None before standalone Search works | Disconnected adapter; optional later MCP stdio leaf uses the same local client | #219 optional |

## Binaries and tooling

| Member | Disposition | Useful responsibility retained | Current mechanisms removed / moved | Primary implementation owner |
|---|---|---|---|---|
| `eliot-searchd` | KEEP / REWRITE | The one daemon composing source, preparation, publication, Qdrant, query, protocol and shutdown | Shadow apps, legacy DIRECT stores, migration/restore/secret/provider/query composition OS and harness entrypoints | #352 sequence |
| `eliot-search` | KEEP / REWRITE | One invocation, owned child/local connection, current typed request, bounded output and exit status | Duplicate core/typed clients, tab/hex/TCP/token endpoints, pairing/grants and opaque handles | #332 |
| `eliot-search-model-worker` | DELETE | None in current refusal stub | Always-unavailable binary/workspace member; future selected model starts from a real measured implementation | Later optional issue |
| `eliot-search-doc-worker` | OPTIONAL | One selected PDFium child/profile if #227 is implemented | Current refusal stub, unlinked custom SHA-256 and unused dependency edges; delete member if deferred | #227 |
| `xtask` | DELETE current package | None after direct Cargo/Git commands unless a substantial surviving custom operation is proven | Generated governance, maps, ledgers, P00/bootstrap/Qdrant source analyser and source-shape validators | #214, #525 |

## Cutover rules

1. A package marked `MERGE / INLINE` or `DELETE` remains only until its last real production caller moves.
2. Do not replace deletion with a forwarding facade, `v2` package, compatibility adapter or copied types.
3. Useful tests move to the receiving owner only when they protect current behavior.
4. Delete package-local instructions, generated maps, workflows and fixtures in the same closure.
5. Run minimal compiler/Clippy over the changed owner and actual immediate consumers after each Rust slice.
6. A runtime/native/Qdrant claim requires execution; this map establishes only the target ownership.

## Remaining uncertainty

The two #598-conditional packages cannot receive a final keep/delete decision until that active handoff is
accepted. Optional packages are deliberately not audited as mandatory product implementations. All other 48
members now have a concrete baseline disposition; implementation may merge more aggressively if the
remaining code does not justify a package boundary.
