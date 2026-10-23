# Analysis registry

Continuation memory for the refactor audit: exact inspected revisions/paths, findings, changes, execution
evidence and remaining uncertainty. It is not a permission gate, package-status database or substitute for owner
instructions.

**Original public baseline:** `c8e38b6fc9829f1cd38070773b1b3e5b21de18ad`  
**Latest inspected source:** `174f3ec758cb0ec5c38b780cd3de913be9120fdc` (`R019`, 2026-10-23)  
**Historical qualification archive:** `2f9c9720f3e14799a0f8dfefa00077d2fa6a9887/qualification`  
**Rule:** resume from the actual PR head, inspect intervening commits, and bind every compiler/runtime claim to
an exact SHA, command and target. Row counts are not a completion percentage.

## Status vocabulary

- **CHECKED** — named static surface inspected for the stated question; not every line/runtime property.
- **PARTIAL** — useful findings exist, but named code/callers/reconciliation remain.
- **PENDING** — insufficient inspection to direct a safe change.
- **RUNTIME-ONLY** — implementation/execution required; prose cannot establish it.
- **PROTECTED** — unpublished work is not inspected/overwritten; consume its accepted public handoff.

## Coverage ledger

| ID | Area | Inspected surface | Status | Current conclusion | Remaining |
|---|---|---|---|---|---|
| A001 | Documentation authority/consistency | Root authority; architecture/implementation; current source, preparation, Qdrant, publication, query, protocol, CLI, maintenance and test owners | PARTIAL | Historical parallel specifications were removed. R009-R019 repeatedly found contradictions and moved algorithms/causal cases back to current owners. | Continue surviving owner/issue sweep only with concrete source inspection; no new global specification. |
| A002 | Historical governance data | Swarm, generated maps, packets/handoffs, context/ticket artifacts | CHECKED | Removed from current tree; Git history is the archive. | Code remnants below. |
| A003 | Workflows/wrappers | Workflows, PowerShell wrappers and temporary verifier | CHECKED | Old gates/wrappers removed; Actions manual-only. | Recheck only when workflows change. |
| A004 | Workspace/package inventory | Root manifest and 51 current members | CHECKED for inventory | Every member has a disposition. R017 leaves an empty `search-retention` shell pending manifest/lock removal. Package count itself is not a target. | Physical Cargo/package cuts. |
| A005 | Entrypoints/runnable product | Daemon/CLI interceptors; one-shot scan; source sync; preparation; DIRECT service; snapshot/sealed/restore/loopback applications | PARTIAL | Snapshot/sealed/restore standalone paths and daemon TCP/token loopback are removed. The primary daemon still has registry/native-root/migration/preparation/DIRECT/secure/final-shell interceptors and no connected source→Qdrant→query path. | One default daemon/CLI application; remove remaining interceptors/compatibility and connect current owners. |
| A006 | Root/config/control | Root owner/config; old journal/snapshot/registry restore; redb snapshot; stopped-root/archive boundary | PARTIAL | One root guard and typed current state selected. Process-held snapshots are not durable build selection; copied external authority is not restore state. | Implement typed state, captured build inputs, Windows lock/durability and archive publication. |
| A007 | Source acquisition/registry/reconciliation | Admission stack; comparator/currentness/catalog/sync; ignore donor | PARTIAL | R010 found wrong-root absence, compound update loss, physical/logical identity mismatch, erased causes, positional work and false currentness. Docs fixed; Rust remains. | Real source/object/current-row cutover, #598 handoff and focused cases. |
| A008 | Private storage/key overlap | Public #598 contract only | PROTECTED | Obsolete six-domain/purge/compatibility programme not inherited. | Consume accepted handoff; do not inspect/reset unpublished work. |
| A009 | Preparation/identity | Materializer profile/request/pipeline/read/model; unitizer/point identity | PARTIAL | Test/resource data contaminates profile identity; budget preflight differs from execution; empty input refused; reuse/install/retry are distinct. | Implement current preparation and empty/gap/budget/ABA/stale-completion cases. |
| A010 | Qdrant/donors | Bridge; Qdrant 1.19 mutation/readback/BM25/tokenizer/scoped-IDF; old live/oracle fixtures | CHECKED for named static questions | QualifiedGate, local BM25 prototype, stale live gate fixture, in-memory oracle and duplicate REST product half are removed. Causal live-Qdrant cases belong to real bridge/publication. | Connect real path; execute relevance/scale/late-write/rebuild. |
| A011 | Publication/view/cleanup | Publication machines; redb view; late writers; cleanup/restore; old rebuild fixture | PARTIAL | Two-sided reconciliation, immutable retrieval, captured build inputs and isolated-generation recovery selected. Old rebuild proof removed. | Implement crash/view/cleanup/rebuild/restore and execute. |
| A012 | Query/evidence/window | Query stack; validator/projector/exact; continuation/result handles; old indexed/handle fixtures | PARTIAL | Post-allocation admission, collapsed failures, raw-only coordinates, whole-source fences, latest-source rechecks, arbitrary expansion and split lifetimes identified. Old proof/handle fixtures removed. | One pinned `QueryWindow`, direct Qdrant/evidence reader; delete production proof/catalogue families. |
| A013 | Protocol/client/CLI | Provider/line stacks; DIRECT session/output/client; snapshot/loopback/pairing/provider fixtures and daemon loopback source | PARTIAL | Active Cancel, aggregate framing and child cleanup remain deficient. R019 removes daemon TCP/token endpoint, authenticated proxy, child/exchange and feature. CLI/provider-client compatibility and wave4/test pairing code remain. | One capped framed owned-child exchange; remove remaining CLI/provider compatibility and execute platform cases. |
| A014 | Product depth | Orientation/comparison/documents; optional workers/overlay; recipe fixture | PARTIAL | User tasks retained; fixed eleven-recipe registry removed. Deleted scaffolds are not capabilities. | Real producers/consumers after baseline Search. |
| A015 | Tests/governance code | xtask/ledger; structural/lifecycle/snapshot/query/provider removals; mixed migration tests | PARTIAL | R015–R018 removed structural and self-contained proof systems. R019 removes source-local tests embedded in the daemon loopback owner. Mixed migration tests still combine real I/O and source assertions. | Move causal cases; delete remaining obsolete fixtures with owners; run compiler/Clippy. |
| A016 | Donors | redb, Qdrant, Aho-Corasick, Git/ripgrep/ignore, Vec | PARTIAL | Concrete seams only; no whole-donor or upgrade claim. | Revisit only for selected implementation questions. |
| A017 | Qualification tree | Complete tree and representative reports/consumers | CHECKED | Archived in Git; only policy/blocker summary remains. | Runtime ceremony A022. |
| A018 | Auxiliary docs | Package/binary filename sweep and supplements | CHECKED for filename/disposition | Duplicate wave/profile/DIRECT specs removed; distinct diagnostics retained. | Semantic consistency A001. |
| A019 | Issues/PR handoff | #352/#214/#244/#275/#279/#280/#293/#302/#332 and PR through R019 | PARTIAL | Named issues link current owners and exact inspected findings. | Update only on owner/source changes; no whole-queue claim. |
| A020 | Runtime evidence | Windows, blocked I/O, live Qdrant, relevance/scale, reader growth, restart/restore, child/GC/archive | RUNTIME-ONLY | Deleted fixture claims are not evidence. Static findings define required runs. | Implement/run exact SHA/commands/raw outcomes. |
| A021 | Compiler baseline | Ubuntu Rust 1.98 runs through `39872058276` | PARTIAL | Latest verified run covers bridge lib at `7d92fb8b`; earlier broad daemon check failed. R009–R019 have no new compiler claim. R019 changes production Rust/Cargo but no local Cargo or workflow dispatch was available. | Run daemon/current consumers and Windows as soon as executable tooling is available. |
| A022 | Runtime qualification/profile identity | Qdrant gate; lexical fixture identity; materializer analogue | PARTIAL | Qdrant gate/process fixtures gone. Lexical fixture-backed activation and profile/test coupling remain. | Remove ceremony through real callers; retain causal examples as tests. |
| A023 | Local GC/storage diagnostics | Legacy maintenance/status/query callers; generic retention source | PARTIAL | Generic sweep/purge source gone; typed stopped-root collector and query-time inventory removal unimplemented. | Implement mark/sweep and remove recursive query/output inventory. |
| A024 | Backup/restore/import | Generic retention/restore removed; #243/#244 | PARTIAL | Target is complete archive plus fresh destination/Qdrant generation. | Implement partial/corrupt/interrupted/protected cases; remove empty Cargo shell. |
| A025 | Feature/harness topology | Root/daemon/CLI manifests; wave ladder; standalone/proof targets; daemon loopback feature | PARTIAL | Standalone targets, lifecycle implementation, nine proof suites and daemon `legacy-loopback-harness` are removed. Wave/dependency shells, CLI loopback compatibility and old production modules remain. | Dedicated Cargo/production cuts with metadata/check/Clippy. |

## Remaining work queue

1. **Cargo shell cut:** remove `wave7-lifecycle`, `full-baseline`, empty `search-retention` root/daemon/lock entries;
   then collapse `wave1`–`wave6` with actual consumers.
2. **CLI/provider cut:** remove CLI/provider-client endpoint/token/pairing compatibility and wave4/test provider
   pairing/router/capability code when direct typed handlers replace it.
3. **Entrypoint/source cut:** remove remaining interceptor chain and shared sealed/migration compatibility after
   causal Windows/object cases move; connect source/object/current-row/preparation.
4. **Qdrant/publication:** one runtime; immutable published state; pending/readback/late writers; atomic view,
   fresh-generation rebuild and cleanup.
5. **Query/session:** direct scoped BM25; pinned evidence; one `QueryWindow`; framed terminal; active Cancel and
   owned-child shutdown; delete production proof/catalogue stacks.
6. **Maintenance/archive/import:** typed stopped-root GC; archive marker last; fresh destination/generation.
7. **Residual tests/qualification:** mixed migration assertions, lexical fixture activation, quarantine/old-command
   fixtures and obsolete consumers.
8. **Product depth/runtime evidence:** exact/navigation/comparison/documents/MCP, then Windows/Qdrant/restart/
   scale/GC/archive/default-binary smoke.

## Pass log

### R001–R014

Removed parallel governance/specification systems, xtask/ledger, 66 structural tests and stale qualification;
audited Qdrant/lexical, source currentness, preparation/publication, evidence/window/transport and GC/restore.
Exact earlier commits and execution scope remain in Git history and issue references.

### R015 — structural/lifecycle proof consumers

Removed ten source-text tests and five restore/retention/purge proof suites (123,096 source bytes).

### R016 — isolated snapshot/sealed applications

Removed eight explicit snapshot/sealed applications, private snapshot/control/local-BM25 source and structural
isolation test (5,765 lines).

### R017 — restore and generic lifecycle implementation

Removed daemon restore source and generic retention/purge/sweep/restore code (3,218 production Rust lines),
leaving a temporary empty manifest/lock shell.

### R018 — disconnected query/provider/rebuild fixtures

Removed nine process proofs (6,740 lines / 243,346 bytes): in-memory query oracle, manual DIRECT/Qdrant join,
rebuild/pin/reclaimer, stale qualification, pairing/provider loopback, recipe registry and random handles. Moved
causal cases to current owner map.

### R019 — remove daemon TCP/token loopback owner (2026-10-23)

Inspected exact head `174f3ec758cb0ec5c38b780cd3de913be9120fdc`:

- daemon `entry.rs` and `Cargo.toml`;
- `authenticated_proxy.rs`, kernel facade and all 11 kernel files;
- `endpoint.rs`, kernel facade and all seven endpoint kernel files;
- `proxy_child.rs` and seven child files;
- `proxy_exchange.rs` and five exchange files;
- provider-composition cfg map;
- current daemon test directory and caller searches after R018;
- protocol/CLI owner docs and issues.

Confirmed:

1. `authenticated_proxy` is reachable only through the feature-gated interceptor in `entry.rs`.
2. The endpoint/proxy chain implements loopback TCP, token/pairing proofs, replay envelopes, child forwarding and
   multi-line terminal parsing selected for deletion.
3. After R018, no current integration test path names/imports `endpoint` or `authenticated_proxy`; embedded tests
   live inside the owner itself and prove only that obsolete transport.
4. General child lifetime/partial-output/cancellation cases remain required but belong to the new owned-child
   framed protocol, not this TCP/token stack.
5. `provider_composition` still referenced `legacy-loopback-harness` in cfg clauses; removing the feature requires
   narrowing those clauses to test/wave4 only to avoid an undefined-feature cfg.
6. CLI/provider-client loopback compatibility is separate and remains; R019 does not silently delete it or claim
   the selected new protocol exists.

R019 removes the daemon feature, interceptor, authenticated proxy, endpoint, proxy-child and proxy-exchange source
(36 files) and narrows provider cfgs. It changes production Rust/Cargo but no dependency or lockfile. No workflow
run exists and no compiler/Clippy/runtime result is claimed; static caller inspection is not build evidence.
