# Package cleanup register

## Package register

The 47-member listing and reverse manifest-edge counts originated at f3d71013 after the first package cleanup;
the counts are historical navigation, not a refreshed dependency census or evidence of runtime consumers.
Root/control/binary dispositions below now reflect the qualified Windows debug root source ad8cc9e8.
S1 is qualified in its recorded Windows debug scope. P1 connects retained-only preparation/current outcome;
source gates/native checks passed with exact scopes in the registry; merge is pending and Q1 is next.
Cargo manifests remain the dependency authority; inspect actual consumers before deletion.
[Work register](ANALYSIS_REGISTRY.md#work-register) owns order/status; [code map](../implementation/CODE_MAP.md)
locates entrypoints. A row is a concrete disposition, not a demand to keep one crate per responsibility.

| Package / actual path | Recorded reverse manifest edges | Cut | Required disposition |
|---|---:|---|---|
| [search-contracts](../../crates/search-contracts/Cargo.toml) | 44 | S1/U1 | Reduce shared consumed values; delete plan/grant/receipt families with consumers. |
| [search-domain](../../crates/search-domain/Cargo.toml) | 36 | F1→X1 | Move actual rules to state/effect owners; delete the global algebra. |
| [search-ports](../../crates/search-ports/Cargo.toml) | 34 | F1→X1 | Replace each real call directly; remove catalogue and crate after last edge. |
| [search-config](../../crates/search-config/Cargo.toml) | 21 | F1 | Keep typed settings actually read by constructors; remove registry/digests. |
| [search-runtime-owner](../../crates/search-runtime/search-runtime-owner/Cargo.toml) | 1 | F2 | Four plain root operations, stable lock and manifest-last publication retained; old owner/key/epoch implementation removed. Windows debug root scope qualified at ad8cc9e8; see registry, not an installed-baseline claim. |
| [search-os-secrets](../../crates/search-runtime/search-os-secrets/Cargo.toml) | 2 | F2/U1 | Remove public legacy key/pairing callers, then package; preserve user keys. |
| [search-control-redb](../../crates/search-control-redb/Cargo.toml) | historical | F2/S1/P1 | Plain-root metadata, guarded current-source rows and current preparation/settings/outcome CAS share control/control.redb on redb 4.5.0. Native read-only capture and persistent-savepoint/empty-probe safety remain. |
| search-source-admission | removed in S1 cut | Disconnected pre-release selection and exclusion framework removed with its callers. |
| search-source-admission-receipt | removed in S1 cut | Receipt-only owner and exclusive fixtures removed. |
| search-source-provenance | removed in S1 cut | Disconnected provenance and admission-archive owners removed. |
| search-source-records | removed in S1 cut | Legacy schema, replay codecs, restore formats, and exclusive fixtures removed. |
| search-source-history | removed in S1 cut | Disconnected revision and registry replay owner removed. |
| [search-source-registry](../../crates/search-source/search-source-registry/Cargo.toml) | historical | S1/P1 | CurrentSourceRecord, Source/Prepared ObjectRef, NativeFileKey and shared PreparationSettings/bounds; CONTROL owns durable revisions/CAS. |
| search-source-identity | removed in S1 cut | Disconnected legacy identity/receipt owner removed; current native identity comes from the opened file handle. |
| search-source-reconcile | removed before R1 | No S1 prerequisite. Future reconciliation remains an R1 work-register obligation after the connected query path. |
| [search-safe-reader](../../crates/search-source/search-safe-reader/Cargo.toml) | current | One bounded explicit-path read with actual Unix/Windows handle identity and before/after stability checks. |
| [search-revision-store](../../crates/search-source/search-revision-store/Cargo.toml) | current | S1/P1 | One plain immutable Source/Prepared writer and verified readback with explicit bounds; no encrypted/profile API. |
| [search-materializer](../../crates/search-prep/search-materializer/Cargo.toml) | historical | P1 | Exact retained UTF-8 and attached line inventory; legacy product/receipt routes retired. |
| [search-unitizer](../../crates/search-prep/search-unitizer/Cargo.toml) | historical | P1 | Complete stored representation/ranges with borrowed decoder and actual prepare-source consumer; old V3/VerifiedUnitSet APIs retired. |
| [search-code-enricher](../../crates/search-prep/search-code-enricher/Cargo.toml) | 1 | O1 | Deferred; check actual feature/callers, remove dormant framework, not a promised parser. |
| [search-lexical](../../crates/search-lexical/Cargo.toml) | 3 | Q1 | Keep consumed text/profile meaning; remove local postings/scoring/alias inflation. |
| [search-projection-planner recipe](../../crates/search-index-qdrant/search-projection-planner/FUNCTIONS.md) | no current package | Q1 | Legacy src/Cargo/fixtures retired in P1. Canonical FUNCTIONS/AGENTS retained; implement bounded real text/page assembly with publication. |
| [search-point-identity](../../crates/search-index-qdrant/search-point-identity/Cargo.toml) | 3 | Q1 | Keep stateless collision-checked generation IDs; no baseline occurrence registry. |
| [search-qdrant-supervisor](../../crates/search-index-qdrant/search-qdrant-supervisor/Cargo.toml) | 1 | Q1 | Remove constructor/binding coupling, then crate; server is operator-owned. |
| [search-qdrant-bridge](../../crates/search-index-qdrant/search-qdrant-bridge/Cargo.toml) | 2 | Q1 | Keep one private SDK client and real I/O; remove qualification/lease/fallback wrappers. |
| [search-publication recipe](../../crates/search-index-qdrant/search-publication/FUNCTIONS.md) | no current package | Q1/R1 | Legacy admission/machine/restore src/Cargo/fixtures retired in P1. Canonical FUNCTIONS/AGENTS retained for actual fixed input/current/evidence/view/cleanup implementation. |
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
| [eliot-searchd](../../bins/eliot-searchd/Cargo.toml) | 0 | F1/F2/S1/P1 | Root, retain-file and retained-only prepare-source handlers connected; old binary routes/fixtures removed. Qdrant and framed queries remain unconnected. |
| [eliot-search](../../bins/eliot-search/Cargo.toml) | 0 | F1/F2/S1/P1; U1/X1 next | Same owned child, bounded validated root/source/stored-unit output and parsed request binding. Framed search/live sessions remain future consumers. |
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
