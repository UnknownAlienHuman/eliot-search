# Package cleanup register

## Package register

All **47 remaining explicit workspace members** are listed once below. Inspected base: `f3d71013`;
Cargo metadata was read on Windows/Linux from that exact revision, then the five removed members were excluded.
The reverse count includes direct normal/dev/target/optional **manifest edges**, not proof of a running caller.
Cargo manifests remain the dependency authority; recheck actual dispatch/features before each deletion.
[Work register](ANALYSIS_REGISTRY.md#work-register) owns order/status; [code map](../implementation/CODE_MAP.md)
locates entrypoints. A row is a concrete disposition, not a demand to keep one crate per responsibility.

| Package / actual path | Remaining direct reverse manifest edges | Cut | Required disposition |
|---|---:|---|---|
| [search-contracts](../../crates/search-contracts/Cargo.toml) | 44 | S1/U1 | Reduce shared consumed values; delete plan/grant/receipt families with consumers. |
| [search-domain](../../crates/search-domain/Cargo.toml) | 36 | F1→X1 | Move actual rules to state/effect owners; delete the global algebra. |
| [search-ports](../../crates/search-ports/Cargo.toml) | 34 | F1→X1 | Replace each real call directly; remove catalogue and crate after last edge. |
| [search-config](../../crates/search-config/Cargo.toml) | 21 | F1 | Keep typed settings actually read by constructors; remove registry/digests. |
| [search-runtime-owner](../../crates/search-runtime/search-runtime-owner/Cargo.toml) | 1 | F2 | Keep plain root, one lock and shared file publication; remove owner epochs/keys. |
| [search-os-secrets](../../crates/search-runtime/search-os-secrets/Cargo.toml) | 2 | F2/U1 | Remove public legacy key/pairing callers, then package; preserve user keys. |
| [search-control-redb](../../crates/search-control-redb/Cargo.toml) | 1 | S1/Q1 | Keep typed current rows/read snapshots; delete journal/replay/receipt platform. |
| [search-source-admission](../../crates/search-source/search-source-admission/Cargo.toml) | 7 | S1 | Keep bounded selection/exclusions around one read; inline small evaluator. |
| [search-source-admission-receipt](../../crates/search-source/search-source-admission-receipt/Cargo.toml) | 1 | S1 | Delete with provenance and original-input consumers; no substitute receipt. |
| [search-source-provenance](../../crates/search-source/search-source-provenance/Cargo.toml) | 4 | S1 | Move real source facts; delete admission archives and this forwarding layer. |
| [search-source-records](../../crates/search-source/search-source-records/Cargo.toml) | 2 | S1 | Move current row codecs to control/source owner; delete old restore formats. |
| [search-source-history](../../crates/search-source/search-source-history/Cargo.toml) | 1 | S1 | Remove replay/history wrappers after registry calls move. |
| [search-source-registry](../../crates/search-source/search-source-registry/Cargo.toml) | 2 | S1/R1 | Keep guarded current source operations; remove event replay and facade. |
| [search-source-identity](../../crates/search-source/search-source-identity/Cargo.toml) | 4 | S1 | Keep source versus content identity; no identity authority or receipt engine. |
| [search-source-reconcile](../../crates/search-source/search-source-reconcile/Cargo.toml) | 1 | R1 | Keep bounded authoritative inventory and desired-state comparison; no watcher ledger. |
| [search-safe-reader](../../crates/search-source/search-safe-reader/Cargo.toml) | 2 | S1 | Keep stable bounded byte acquisition and cancellation; remove supplied authority. |
| [search-revision-store](../../crates/search-source/search-revision-store/Cargo.toml) | 1 | S1 | Keep one plain object writer/reader; remove duplicate/protected-store protocols. |
| [search-materializer](../../crates/search-prep/search-materializer/Cargo.toml) | 4 | P1 | Keep one retained transform and current outcome; no second artifact writer. |
| [search-unitizer](../../crates/search-prep/search-unitizer/Cargo.toml) | 4 | P1 | Keep one representation plus bounded ranges; no repeated parsing/manifests. |
| [search-code-enricher](../../crates/search-prep/search-code-enricher/Cargo.toml) | 1 | O1 | Deferred; check actual feature/callers, remove dormant framework, not a promised parser. |
| [search-lexical](../../crates/search-lexical/Cargo.toml) | 3 | Q1 | Keep consumed text/profile meaning; remove local postings/scoring/alias inflation. |
| [search-projection-planner](../../crates/search-index-qdrant/search-projection-planner/Cargo.toml) | 2 | Q1 | Move bounded page assembler beside publisher; no public whole-plan/proof model. |
| [search-point-identity](../../crates/search-index-qdrant/search-point-identity/Cargo.toml) | 3 | Q1 | Keep stateless collision-checked generation IDs; no baseline occurrence registry. |
| [search-qdrant-supervisor](../../crates/search-index-qdrant/search-qdrant-supervisor/Cargo.toml) | 1 | Q1 | Remove constructor/binding coupling, then crate; server is operator-owned. |
| [search-qdrant-bridge](../../crates/search-index-qdrant/search-qdrant-bridge/Cargo.toml) | 2 | Q1 | Keep one private SDK client and real I/O; remove qualification/lease/fallback wrappers. |
| [search-publication](../../crates/search-index-qdrant/search-publication/Cargo.toml) | 1 | Q1/R1 | Keep fixed pending input/current view/cleanup; remove duplicate effect journals. |
| [search-epoch-pins](../../crates/search-index-qdrant/search-epoch-pins/Cargo.toml) | 5 | R1 | Move minimal view guard/manager to publisher/query; remove generic pin platform. |
| [search-index-reclaimer](../../crates/search-index-qdrant/search-index-reclaimer/Cargo.toml) | 2 | R1 | Move whole-collection deletion to publisher/bridge; remove point-deletion framework. |
| [search-access](../../crates/search-query/search-access/Cargo.toml) | 7 | U1/X1 | Inline local source selection; remove grants/security barriers. |
| [search-overlay](../../crates/search-query/search-overlay/Cargo.toml) | 1 | O1 | Deferred; remove dormant editor framework after feature/caller check. |
| [search-exact](../../crates/search-query/search-exact/Cargo.toml) | 1 | X1 | Keep retained literal scan; remove proof/planning/completeness wrappers. |
| [search-subject-resolver](../../crates/search-query/search-subject-resolver/Cargo.toml) | 2 | O1 | Selected depth only; retain only real source-backed subject lookup when connected. |
| [search-query-planner](../../crates/search-query/search-query-planner/Cargo.toml) | 3 | U1 | Replace with direct dispatch; delete one-leg plans and crate if no algorithm remains. |
| [search-retrieval-executor](../../crates/search-query/search-retrieval-executor/Cargo.toml) | 1 | U1 | Keep direct bounded request execution; no tickets/scheduler/history service. |
| [search-candidate-validator](../../crates/search-query/search-candidate-validator/Cargo.toml) | 2 | U1 | Keep pinned evidence/object/range validation; no echoed-proof authority. |
| [search-comparator](../../crates/search-query/search-comparator/Cargo.toml) | 1 | O1 | Selected depth only; no placeholder comparisons or generic proof algebra. |
| [search-handles](../../crates/search-query/search-handles/Cargo.toml) | 3 | X1 | Remove second handle catalogue; emitted item locators belong to QueryWindow. |
| [search-result-projector](../../crates/search-query/search-result-projector/Cargo.toml) | 1 | U1 | Keep source-backed cards/order/gaps; remove duplicate rank/proof output layers. |
| [search-continuation](../../crates/search-query/search-continuation/Cargo.toml) | 1 | X1 | Keep one same-session window/cursor; delete duplicated daemon/shared catalogues. |
| [search-retention](../../crates/search-runtime/search-retention/Cargo.toml) | 1 | M1 | Replace with direct stopped-root maintenance; no online local-object GC/lease platform. |
| [search-eval](../../crates/search-eval/Cargo.toml) | 1 | V1 | Keep real product measurements only; no acceptance receipts or runtime authority. |
| [search-provider-protocol](../../crates/search-provider-protocol/Cargo.toml) | 4 | U1/X1 | Keep one bounded typed frame; remove pairing/MAC/grant/replay protocol. |
| [search-provider-client](../../crates/search-provider-client/Cargo.toml) | 1 | U1/X1 | Keep owned-child serviced I/O; remove duplicate clients, discovery and old credentials. |
| [eliot-searchd](../../bins/eliot-searchd/Cargo.toml) | 0 | F1→M1 | Keep actual entry.rs composition; replace old dispatch and remove displaced modules. |
| [eliot-search](../../bins/eliot-search/Cargo.toml) | 0 | F1/U1/X1 | Keep actual entry.rs invocation/presentation; no parallel legacy client. |
| [search-os-secrets-windows](../../crates/search-runtime/search-os-secrets-windows/Cargo.toml) | 2 | F2/U1 | Move needed file/OS operations first; remove credential/DPAPI wiring, not user data. |
| [search-provider-transport-windows](../../crates/search-runtime/search-provider-transport-windows/Cargo.toml) | 2 | U1 | Remove unused listening-pipe route with client imports; stdio uses no second transport. |

## Removed in this cleanup

`xtask`, `search-revision-crypto`, `search-eliot-adapter`, `eliot-search-model-worker` and
`eliot-search-doc-worker` had no external Cargo consumer. Full-tree searches found no surviving product
constructor/import/launch for the three optional adapter/worker targets or the crypto package. Worker calls
existed only in their own unavailable fixtures; those leave with the binaries. No useful model/PDF/MCP operation
was implemented by the deleted stubs. Optional product depth remains in the [product scope](../product/AGENT_ANALYSIS_FRAMEWORK.md#optional-depth).

The xtask consumer closure (old scripts/workflows/swarm/config/qualification inputs) was removed together.
Historical sources and donor investigations remain in [the immutable pre-cleanup tree](https://github.com/UnknownAlienHuman/eliot-search/tree/f3d710131aa3b06a63198694bed7ec82b1e251df).
User files, external server data, existing protected roots/keys and other branches/worktrees were not deleted.

## Finish a row

Move the named current consumer, remove old imports/module/feature/Cargo edges and exclusive fixtures, then
run the [minimal actual-consumer check/Clippy](../implementation/WORKING_GUIDE.md#minimal-production-checks).
Remove the row only with the workspace member, or update it to the final retained responsibility. Zero reverse
manifest edges alone do not justify deleting a binary or an externally launched adapter; trace launch paths too.
No replacement facade, dormant feature flag or duplicate package map is required.
