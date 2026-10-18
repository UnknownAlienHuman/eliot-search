# eliot-searchd — composition and command map

Follow the [implementation guide](../../docs/implementation/README.md). The daemon composes real effects
and handlers; library owners retain data/algorithm responsibilities. This map describes public source at
`c8e38b6` and replacement work. It does not freeze old commands, receipts or package boundaries.

## Current commands and missing connections

| Existing path | What the source actually does | Next implementation |
|---|---|---|
| [entry.rs](src/entry.rs) | Chains `maybe_refuse_legacy_effects` and several handlers which recapture arguments. | Parse one invocation/config value and dispatch one concrete operation. |
| native root / direct registry | Opens current root and redb metadata. | Use one root owner and current-state transactions. Root registration is not ingestion. |
| `--admit-source-v2` | Reads, hashes and classifies the file, commits admission metadata, then drops the body and returns `retained_revision:false`. | Pass the same bytes into the immutable writer, commit source head and prepare them. |
| `--sync-source-roots` | Successful preflight ends with `SOURCE_ROOT_SYNC_EFFECTS_UNAVAILABLE`. | Execute bounded inventory/update through the same ingestion owner. |
| legacy preparation commands | Prepare old retained objects through a second storage engine. | Reuse useful materialization/coordinates with the one current reader/writer. |
| public DIRECT service | Serves existing read-only DIRECT commands; mutation unavailable. | Keep exact source scan explicit and connect the current typed protocol. |
| indexed search | `query_composition.rs` uses a process-test oracle; live `BlockingRealQueryPlane` is not called. | Connect Qdrant BM25, pinned view-manifest lookup and retained excerpts. |
| query recipe composition | `registry.rs`, `service.rs`, `serving.rs` and `live_authority.rs` total about 50 KiB. The registry trait has no real `RegisteredAdmittedRecipeHandler` implementation in public main. | Replace capability/grant/recipe scaffolding with direct current typed handlers. Keep useful request lifecycle and real restricted-client checks separately. |
| result paging | Public query stores `StoredMatch` values in one continuation catalog, then mints another result handle per item and rescans the source catalogue. | Use one `QueryWindow` with pinned view, final order, cursor, selected-page reads and optional item locator. |

Do not expose test-only index/mutation methods by removing `cfg(test)` and call that integration.

## Composition responsibilities

| Owner | Concrete responsibility |
|---|---|
| Invocation/root | One parsed command, one root lock, actual settings and orderly shutdown. |
| Source / revision store | Stable acquisition and one immutable writer/reader. Bytes precede source-head commit. |
| Preparation | One materialization and structure-first units. Unsupported transformations become gaps. |
| redb control | Current settings/source/head/artifact/publication records in short transactions. No source bodies or query history. |
| Publication | One pending artifact, Qdrant effect/readback and one visible `PublishedView`. |
| Qdrant/query | One private connection/runtime, scoped query, view-manifest lookup and retained-byte validation. |
| Query window | One process-local cursor/view guard for paging and bounded expansion; restart may expire it. |
| Local protocol | One bounded typed request/result exchange. Actual handlers determine availability. |

### Direct request dispatch

Replace frozen eleven-recipe and parallel evidence endpoints with:

```text
Search    ranked source-backed evidence
Exact     one bounded complete source scan
Continue  next page from QueryWindow
Expand    selected item/range from QueryWindow
Orient    optional source guide after facts are connected
Compare   optional source-backed comparison after facts are connected
```

The local CLI calls these handlers directly over owned stdio/pipe. It needs no capability registry,
descriptor digest, grant transcript or plan fingerprint. A selected restricted-client profile may
advertise the handlers actually present and must enforce its real connection/scope policy separately.

Planner input is resolved scope + pinned view + query + real service references + limits. Do not create
`SearchTaskPlan` or `CapabilitySet` proofs. If lexical search has one branch, call it directly.

### Failure and external outcomes

A successful redb abort before commit is a known failure. An unresolved commit or possible Qdrant write
has a real recovery obligation. Keep pending publication for that external effect; do not extend durable
receipts to reads, planning, paging or pure transformations. Qdrant/source work runs outside the redb writer.

## Remove replaced paths

- migration/cutover/rollback frameworks for unsupported old roots;
- unused DEK/secret-lifecycle machinery outside a selected storage profile;
- legacy loopback/token-file transports and duplicate applications;
- V1 recipe registry, V2 parallel endpoint, capability projection and canonical query-host ceremony from
  the trusted-local path;
- duplicate result-handle/continuation stores and their durable lifecycle types;
- daemon Publisher/file journal after one publication owner is connected;
- source-shape tests, package maps, digest ledgers and wave feature gates with their consumers.

Unsupported old roots fail before writes and remain untouched. Do not replace deleted code with adapters,
synthetic grants, fake IDs, approved booleans or another registry.

## Verification

Verify the connected application: retain a real file across restart, prepare/index it, query from CLI and
reopen the correct excerpt. Then check one edit, excluded scope, old pinned view, paging, exact scan, lost
index, cancellation and pending external publication. Unsupported optional operations must fail truthfully.

Use minimal compiler/Clippy for changed production owners and actual consumers. Native/live-Qdrant/scale
claims require actual execution; source presence, capability descriptors and mocks do not establish them.
