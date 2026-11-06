# Package cleanup register

## Package register

The 47-member listing and reverse manifest-edge counts originated at f3d71013 after the first package cleanup;
the counts are historical navigation, not a refreshed dependency census or evidence of runtime consumers.
Root/control/binary dispositions below now reflect the qualified Windows debug root source ad8cc9e8.
S1/P1 are merged and qualified in their recorded Windows debug scopes. Q1 publication/current/evidence/view
is accepted at 3c33d831 for Windows debug, with PR merge pending. U1 retained-excerpt search is next.
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
| [search-control-redb](../../crates/search-control-redb/Cargo.toml) | historical | F2/S1/P1/Q1 | One database owns root/source/preparation plus fixed source/page/evidence/current/cleanup metadata and complete RO PublishedView validation; persistent-savepoint/empty-probe safety remains. |
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
| [lexical policy](../../crates/search-lexical/FUNCTIONS.md) | no current package | Q1 | Rust/Cargo/local postings/numeric BM25/sparse/alias families retired. Canonical Code/BM25 policy retained; bridge implements actual server operations. |
| [search-projection-planner recipe](../../crates/search-index-qdrant/search-projection-planner/FUNCTIONS.md) | no current package | Q1 | Legacy crate retired in P1; Q1 concrete daemon/CONTROL assembler persists complete saved text/pages and evidence. Canonical policy retained. |
| [search-point-identity](../../crates/search-index-qdrant/search-point-identity/Cargo.toml) | 3 | Q1 | Keep stateless collision-checked generation IDs; no baseline occurrence registry. |
| [search-qdrant-supervisor](../../crates/search-index-qdrant/search-qdrant-supervisor/Cargo.toml) | 1 | Q1 | Remove constructor/binding coupling, then crate; server is operator-owned. |
| [search-qdrant-bridge](../../crates/search-index-qdrant/search-qdrant-bridge/Cargo.toml) | current | Q1 | Concrete Backend with private exact SDK1.20/generated clients, wire caps and actual vectorless schema/InsertOnly/readback/query/canary/delete; old qualification/live-launch/oracle families retired. |
| [search-publication recipe](../../crates/search-index-qdrant/search-publication/FUNCTIONS.md) | no current package | Q1/R1 | Q1 daemon/CONTROL implements actual fixed input/current/evidence/view/cleanup. Canonical FUNCTIONS/AGENTS retain broader convergence/uncertain-effect/view-lifetime policy. |
| [search-epoch-pins](../../crates/search-index-qdrant/search-epoch-pins/Cargo.toml) | 5 | R1 | Move minimal view guard/manager to publisher/query; remove generic pin platform. |
| [cleanup policy](../../crates/search-index-qdrant/search-index-reclaimer/FUNCTIONS.md) | removed in Q1 | Q1/R1 | Rust/Cargo/exclusive fixtures retired; actual whole-collection deletion/absence is bridge-owned with publisher/CONTROL guards. |
| [search-access](../../crates/search-query/search-access/Cargo.toml) | 7 | U1/X1 | Inline local source selection; remove grants/security barriers. |
| [overlay policy](../../crates/search-query/search-overlay/FUNCTIONS.md) | removed in Q1 | O1 | Disconnected Rust/Cargo/editor framework and exclusive fixtures retired; optional selected editor policy remains. |
| [search-exact](../../crates/search-query/search-exact/Cargo.toml) | 1 | X1 | Keep retained literal scan; remove proof/planning/completeness wrappers. |
| [search-subject-resolver](../../crates/search-query/search-subject-resolver/Cargo.toml) | 2 | O1 | Selected depth only; retain only real source-backed subject lookup when connected. |
| [search-query-planner](../../crates/search-query/search-query-planner/Cargo.toml) | 3 | U1 | Replace with direct dispatch; delete one-leg plans and crate if no algorithm remains. |
| [retrieval policy](../../crates/search-query/search-retrieval-executor/FUNCTIONS.md) | removed in Q1 | U1 | Old scheduler/leg/receipt source and fixtures retired; actual Q1 nominations are direct Backend calls, with read-only excerpt consumer next. |
| [search-candidate-validator](../../crates/search-query/search-candidate-validator/Cargo.toml) | 2 | U1 | Keep pinned evidence/object/range validation; no echoed-proof authority. |
| [search-comparator](../../crates/search-query/search-comparator/Cargo.toml) | 1 | O1 | Selected depth only; no placeholder comparisons or generic proof algebra. |
| [search-handles](../../crates/search-query/search-handles/Cargo.toml) | 3 | X1 | Remove second handle catalogue; emitted item locators belong to QueryWindow. |
| [search-result-projector](../../crates/search-query/search-result-projector/Cargo.toml) | 1 | U1 | Keep source-backed cards/order/gaps; remove duplicate rank/proof output layers. |
| [search-continuation](../../crates/search-query/search-continuation/Cargo.toml) | 1 | X1 | Keep one same-session window/cursor; delete duplicated daemon/shared catalogues. |
| [search-retention](../../crates/search-runtime/search-retention/Cargo.toml) | 1 | M1 | Replace with direct stopped-root maintenance; no online local-object GC/lease platform. |
| [search-eval](../../crates/search-eval/Cargo.toml) | 1 | V1 | Keep real product measurements only; no acceptance receipts or runtime authority. |
| [search-provider-protocol](../../crates/search-provider-protocol/Cargo.toml) | 4 | U1/X1 | Keep one bounded typed frame; remove pairing/MAC/grant/replay protocol. |
| [search-provider-client](../../crates/search-provider-client/Cargo.toml) | 1 | U1/X1 | Keep owned-child serviced I/O; remove duplicate clients, discovery and old credentials. |
| [eliot-searchd](../../bins/eliot-searchd/Cargo.toml) | current | F1/F2/S1/P1/Q1 | Root/retain/prepare/publish handlers and actual Qdrant/current/evidence/view connected. Read-only retained-excerpt search remains U1. |
| [eliot-search](../../bins/eliot-search/Cargo.toml) | 0 | F1/F2/S1/P1/Q1; U1 next | Same owned child and closed bounded root/source/unit/publication metadata responses. Framed search text/citations remain future. |
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
