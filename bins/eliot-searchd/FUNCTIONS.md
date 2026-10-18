# eliot-searchd — composition and command map

Follow the [implementation guide](../../docs/implementation/README.md). The daemon composes the
application's real effects and handlers; library owners retain their data/algorithm responsibilities.
This map describes public source at `c8e38b6` and the concrete replacement work. It does not freeze
old schemas, commands, receipts, platform limits or package boundaries.

## Current commands and missing connections

| Existing path | What the source actually does | Next implementation |
|---|---|---|
| [entry.rs](src/entry.rs) | Chains `maybe_refuse_legacy_effects` and several `maybe_run` handlers; handlers independently capture/strip arguments. | Parse one invocation/config value and dispatch the selected concrete operation. Keep native paths as `OsString`. |
| [Native root command](src/native_root_command.rs), [direct registry](src/owner_composition/kernel/direct_registry.rs) | Initializes/opens the current root and redb registry; root registration/listing updates metadata. Schema-5 and policy archive forms are current format details. | Use one root owner and direct current-state transactions. Root registration is not retained-source ingestion. |
| `--admit-source-v2`: [native source](src/owner_composition/kernel/native_source.rs), [command](src/owner_composition/kernel/native_source_command.rs) | Reads a regular NTFS file, hashes/classifies the body, commits admission metadata and returns `retained_revision:false`. `AcquiredNativeSource` does not carry the body forward. | Pass those same bytes to the immutable revision writer, then commit the source head and prepare that retained revision. |
| `--sync-source-roots`: [command](src/secure_commands/kernel/source_sync.rs), [preflight](src/owner_composition/kernel/source_sync.rs) | Successful preflight still ends with `SOURCE_ROOT_SYNC_EFFECTS_UNAVAILABLE`; no ingestion effect follows. | Replace the preflight-only custody/callback chain with real bounded sync through the same ingestion operation. |
| `--prepare-revision` / `--prepare-root`: [preparation composition](src/preparation_composition.rs), [batch](src/preparation_store/kernel/batch.rs) | Prepares existing legacy retained revisions through `persist_canonical`; it does not retain newly admitted native sources. | Reuse useful materialization/coordinate logic with the new reader. Old storage layout and extended receipt output are not prerequisites. |
| `--serve-data-root`: [runtime entry](src/public_runtime_service/kernel/entry.rs), [dispatch](src/public_runtime_service/kernel/dispatch.rs) | Provides the current read-only DIRECT service. Mutation commands return `SERVICE_MUTATION_UNAVAILABLE`; old mutation handlers are test-only. | Connect supported typed local requests to actual source/query handlers. Keep DIRECT explicitly named. |
| `--search-root`, list/verify/read commands: [read commands](src/secure_commands/kernel/read.rs) | Existing native read routes consume retained DIRECT state. | Keep useful exact reads/diagnostics through the one application; remove duplicate dispatch and format plumbing. |
| Indexed search: [query composition](src/query_composition.rs) | `execute_pinned_leg` still receives a process-test port; the normal public indexed path is unconnected. | Connect the existing executor and `BlockingRealQueryPlane`, then resolve candidates to retained bytes and useful excerpts. |

Legacy `--index-file`, `--index-directory`, retire/repair/GC commands are explicitly refused by
[secure command support](src/secure_commands/kernel/support.rs). Existing index/retire methods in
[secure_direct_store/kernel/catalog.rs](src/secure_direct_store/kernel/catalog.rs) and the entire
[public mutation module](src/public_runtime_service/kernel/mutation.rs) are test-only. Do not remove
`cfg(test)` and claim that the new ingestion/storage path is integrated.

## Composition responsibilities

| Owner | Concrete responsibility |
|---|---|
| Daemon invocation/root lifetime | One parsed command, one OS-owned root lock, actual configuration and orderly shutdown. Retire repeated argv fingerprints, executable hashing and per-ABI work-accounting scaffolding. Retire the old writer's co-lock with that writer. |
| Source / `search-revision-store` | Stable source acquisition and one persistent immutable writer/reader. Bytes precede the source-head transaction; a source-record revision is not a content revision. Do not reread a path between stages or invent binding IDs. |
| `search-materializer` / `search-unitizer` | Prepare retained bytes and feed the existing UnitSet/planner path. Current exact UTF-8 profiles require byte-identical coordinates; unsupported transformations produce a real gap. |
| `search-control-redb` | Current settings/source/head/artifact/publication records in short transactions. No source bodies, ranked-query history or lifetime archive of completed operation receipts. |
| `search-publication` | The sole publication transition owner. Daemon persists its pending intent, performs actual Qdrant effects/readback, then commits/publishes the visible view. Remove the duplicate [daemon Publisher](src/publication_composition/kernel/publisher.rs) and file journal. |
| Qdrant bridge / query owners | One private connection/runtime, actual shared payload/profile/scope contracts, coherent query pins and retained-byte validation. Do not manufacture qualification tokens, fixture IDs or successful mock receipts. |
| Client service | Bounded framed request/result exchange on an owned local connection; actual handlers determine availability. One absolute deadline spans the operation. Request windows/pins are transient. |

A successful redb abort before commit is a known failure/cancellation, not an unknown commit.
An unresolved commit or possible Qdrant write has a real recovery obligation. Keep pending external
publication separate; do not extend that state to ordinary reads and pure transformations.
Qdrant calls and source/preparation work run outside the redb writer.

### Readiness and client boundary

The CLI's [public client](../eliot-search/src/public_client.rs) already starts an owned daemon child
on stdio. Use that concrete connection for the baseline and replace tab/hex/substr parsing with one
typed framed exchange. Mandatory pairing/MAC/grants are removed from trusted-local constructors
with their producers/consumers; no synthetic grant or "allowed" value fills an old API.

The [StandaloneLocalOwner seam](src/access_composition/native_binding_listener/local.rs) has no
concrete `StandaloneLocalListener` implementation/caller in the inspected source. Its presence is
not a working listener and it is not a prerequisite for the child-stdio path. A persistent owner-only
pipe or genuinely restricted client can extend the same service when needed. See the
[CLI map](../eliot-search/FUNCTIONS.md#current-source-and-missing-linkage).

Report the actually available route and any concrete gap. A healthy Qdrant process, feature flag,
compiled constructor or closed issue does not establish indexed serving. Ordinary top-k remains
non-exhaustive; exact counts/absence checks are separately requested work.

## Remove replaced paths

- [Source migration commands](src/source_migration_command.rs) and `control_migration_*` stage offline
  artifacts; they do not complete normal serving cutover. Remove the general plan/bootstrap/import/
  cutover/rollback path with its last consumers. An old unsupported root gets an error before writes
  and stays intact; neither legacy support nor an export implementation precedes the new product.
- [Native DEK commands](src/owner_composition/kernel/native_dek_command.rs) and
  [selection](src/owner_composition/kernel/native_dek_selection.rs) manipulate key metadata.
  They do not make source admission a connected encrypted retained-source path. Optional encryption
  uses one real writer/reader when selected; remove unused DEK/secret-lifecycle machinery.
- Retire mandatory six-domain/provider-transfer and same-user receipt chains. Keep actual explicit
  source scope, OS connection ownership, useful source/coordinate validation and private Qdrant access.
- Remove legacy loopback/token-file transports, duplicate old application handlers and their exclusive
  fixtures. Preserve useful DIRECT behavior through current owners, not a parallel legacy application.
- Delete source-shape tests, coverage/status/digest ledgers and delivery-wave feature gates with their
  retired consumers. Do not replace them with another approval registry or wrapper layer.

### Failure and qualification evidence

Verify the integrated application: retain a real file across restart, prepare/index it, find it from
the CLI and reopen the correct excerpt. Then check an edit, missing revision, excluded scope, lost
index, cancellation and pending external publication. Check that root exclusion and bounded request
cleanup work. An old unsupported root and sole retained data remain untouched.

Use meaningful errors and relevant compiler/behavior checks selected by the
[implementation guide](../../docs/implementation/README.md#7-verification-and-delivery).
No migration golden, original-invocation receipt, native clear counter, stage denominator or historical
qualification badge is a requirement for the replacement. Native/live-Qdrant/scale claims need actual
execution of that behavior; mocks and source presence do not establish them.
