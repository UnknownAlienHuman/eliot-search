# eliot-searchd — one application, one owner chain

Follow the root [architecture](../../docs/architecture/README.md) and
[implementation](../../docs/implementation/README.md). Existing command/package/receipt boundaries are not
compatibility requirements. The [analysis registry](../../docs/audit/ANALYSIS_REGISTRY.md) records inspected
revisions and unfinished checks; it is not a second implementation specification.

## Current problem

The binary still contains several applications selected by an ordered interceptor chain:

```text
legacy effect refusal
→ registry/journal command
→ native-root lifecycle
→ migration
→ legacy preparation
→ DIRECT session
→ secure command stack
→ final shell
```

Each captures arguments/config and may shadow later handlers. The CLI's persistent child can therefore reach
another implementation or refusal instead of the named service. Historical wave features compile dependencies
and modules; they do not construct a connected application.

Standalone snapshot/sealed applications, generic retention/restore, daemon TCP/token proxy, obsolete proof
fixtures and the empty retention Cargo shell have been removed. Their selected replacements are not thereby
implemented. CLI/provider-client compatibility, shared sealed/migration code and production query/rebuild modules
remain. Do not restore the deleted `wave7-lifecycle` or `full-baseline`.

Representative current islands:

- source admission restores/requalifies registry history yet drops acquired bytes;
- source sync still returns `SOURCE_ROOT_SYNC_EFFECTS_UNAVAILABLE`;
- preparation uses another DIRECT store/profile/receipt family;
- indexed query proof modules are not connected to the real default handler;
- DIRECT query uses continuation plus random result-handle catalogues and the old line protocol;
- provider composition still contains wave4/test pairing/grant/envelope code;
- optional dependency aggregation is not evidence of a current handler.

Do not fix reachability by enabling all features, adding another dispatcher, or treating a generic constructor
as a connected service. Trace the callable input to the concrete effects and output, change those consumers
with their producer, then remove the replaced owner. See R021 for the inspected feature/consumer boundary.

## Help, diagnostics and provider-removal boundary

Shell help has one owner in `app/kernel/spec.rs`. `entry.rs` must not append a second help fragment based on
another argv read. Existing legacy command names are labeled as such; their presence in help is not a claim
that the source → Qdrant → CLI path exists. Derive new handler advertisement from the actual constructed
application when that application is connected, not from feature names or a new capability registry.

`--self-test` reports only its in-process checks. It neither opens a retained store nor constructs or queries
Qdrant; passing it cannot advertise persistence, encryption, file/stdin I/O or indexed-search readiness.
Actual handler availability, dependency health, data freshness and the outcome of a diagnostic are different
observations. None becomes a runtime credential.

Do not delete `provider_composition/**` as an undifferentiated directory. The inspected
`source_roots/kernel/inspection.rs` still calls its ungated currentness helpers. Move this small observation
logic with its source/status consumers when removing provider pairing/query code; do not keep the whole
provider platform or introduce a forwarding compatibility crate for it.

The old helper's `index_available` boolean does not establish that published revisions match synchronized
source heads. Its inspected production caller supplies `false`, so R021 establishes a replacement hazard, not
a reproduced stale-success incident. Use the existing source/preparation/published-state comparison for
freshness; replacing that `false` with backend health would be wrong. Do not add a second freshness ledger.

## Retire the native authority platform, not its useful resource ownership

The remaining canonical handshake/work/local-stream, native binding/listener and live-query authority families
are not prerequisites for trusted child stdio. Their removal decision does not require repairing every old
pairing/receipt/registration algorithm. Trace surviving outside callers and shared effects, move the useful
behavior to the selected owner, then remove the exclusive family. Do not construct missing factories just to
make obsolete generic APIs reachable.

[Query](../../crates/search-query/README.md#scope-comes-from-the-published-view) owns the pinned-view and short-lock
rule. Do not carry `LiveNativeQueryAuthorityOwner`'s per-turn population validation or mutex across backend and
socket output into it. Tuning a request work limit is not a security-generation migration or a reason to rebuild
an authority owner; transformation-affecting settings still follow the preparation/profile recipe.

Keep one explicit process/root owner until its real in-process work has stopped. Listener/discovery cleanup
failure must not make that owner unreachable through `mem::forget`. Report the cleanup failure and complete
shutdown through the same owner; do not release the root while work still uses it, invent a RELEASED receipt,
or erase retained user data. The child-stdio baseline needs no endpoint descriptor. Uncertain external writes
remain publication's persisted unfinished work, not a reason to leak an anonymous process owner.

Every retry path in any retained polling adapter, including `Interrupted`, observes the existing deadline and
cancellation; no branch skips its bound indefinitely. Use the selected runtime rather than preserving custom
accept/pairing loops. Shared OS-secret and #598 object/key code are not exclusive provider machinery.

### Inspected callers and effects

R027 inspected the following boundaries at `5af8cde5`. Whole-tree Rust symbol searches below are textual caller
evidence, not a resolved Cargo graph or proof that the binary builds.

| Existing path | Actual caller/effect | Cutover boundary |
|---|---|---|
| `source_migration_command.rs` | `entry.rs` still routes plan/bootstrap/stage migration commands into legacy input/inventory and DIRECT store effects. | Remove this callable route with its last consumers. It is not merely a test harness. Preserve the causal cases mapped in [TEST_CUTOVER](../../docs/implementation/TEST_CUTOVER.md#mixed-migration-tests-retained-for-behavior-extraction); do not implement old-format migration for the new baseline. |
| `native_root_command.rs`, `development/kernel/owner.rs`, `owner_composition/kernel/succession.rs` | Root commands are reachable; development ownership uses `SealedRootLease`; succession reads the sealed owner epoch/head and root identity. | Preserve actual exclusive root ownership and physical root identity in the current root owner. Remove the sealed epoch/receipt programme with its consumers, not the lock that still excludes another writer. |
| `preparation_composition`, `public_runtime_service`, `secure_commands` | Explicit prepare commands still use DIRECT preparation/store owners. The DIRECT service is read-only and its mutation module is test-only; its serve argv is shadowed by direct-registry selection. Secure dispatch refuses old mutations; its health/search/list/verify/read arms are test-only, while the real one-shot read implementation is `secure_commands/kernel/read.rs` (also behind earlier selection). | Change actual command selection and read/write/preparation callers together. A test-only dispatch arm does not make the shared store or protection code dead, and a compiled read implementation is not necessarily the first selected handler. |
| `access_composition/native_binding/{journal_open,startup}.rs` and `process.rs` | `restore_existing_standalone_process` has only a definition/re-export in the inspected tree; its restore chain has no external entrypoint caller. Journal opening retains a file identity and derives a previous owner epoch before restoring registration. | Retire the disconnected restore/registration chain. Keep useful verified-file/root ownership in the direct root/control owner; do not carry provider epochs or build a factory to activate this family. |
| `native_binding/opening/{connections,credential,tcp}.rs` | Standalone TCP/local opening is called within the process family. Windows credential adapters perform real Credential Manager reads/publication; non-Windows paths refuse. | Retire provider pairing/listener policy for trusted child stdio. Preserve the shared OS-secret implementation for actual storage consumers; provider-only credential methods can go with their last callers. |
| `native_binding/registration/{intent,provisioning,readback,finalization}.rs` | Five-row provisioning intents coordinate Credential Manager with redb; finalization publishes policy state and drains/revokes sessions, handles and continuations. | These effects explain what the old family owns, not a requirement to reconstruct it. Remove its exclusive records/callers; direct sessions and query windows own their own bounded lifetime. No new generic registration journal. |
| `revision_protection`, `secure_direct_store`, `search-os-secrets-windows` | Retained reads and DIRECT preparation still share object protection/native secret code outside provider pairing. | Preserve useful protection until the actual object/key owner replaces these callers. Respect #598's unpublished overlap; no competing writer or blanket storage deletion. |

The `provider_composition` source-currentness consumer is described above. Config has its own
[three-path caller map](../../crates/search-config/FUNCTIONS.md#observed-callers-and-startup-refusal).
R028 adds the remaining query/rebuild boundaries below. These established boundaries are implementation inputs,
not an instruction to audit the same obsolete native algorithms again.

### Query, publication and rebuild boundaries

R028 at `37dbb198` traced these modules and their outside references. `entry.rs` already declares query under
`wave4-query` and rebuild under `wave3-index`; publication composition is ungated but allows dead code.
No selected production handler constructs these owners. Textual caller searches do not certify a Cargo build.

| Existing owner | Observed work and consumers | Receiving owner / removal |
|---|---|---|
| `query_composition`, registered service and live authority | `execute_pinned_leg`, registry/service constructors and live-authority construction have no outside production caller. `ProcessTestIndexedPort` queries, reads and counts the in-memory `QdrantBridge`, not `RealDataPlane`. | Connect the direct [query handler](../../crates/search-query/README.md) to a pinned view, concrete SDK and retained evidence, then remove the oracle host, query-specific native provider/capability gates and ticket/proof wrappers. No unconditional count or old qualification credential. |
| `publication_composition::Publisher` | `propose` performs an in-memory floor CAS and writes opaque intent bytes through `FileJournal`; `observe_control_commit` changes that floor. The outside consumer is `publication_fault_process.rs`, which directly includes the module and uses a backend double. No redb current-view commit or Qdrant mutation occurs here. | Use the one [publication recipe](../../crates/search-index-qdrant/search-publication/FUNCTIONS.md) and real control/bridge effects. Retire the competing floor/journal and exclusive fixture; transfer useful failure cases to actual owner behavior. |
| Publication compensation/recovery/retirement helpers | Traits and exact-ID records plus decisions from caller-supplied booleans; no concrete backend observation or physical retirement. | Keep fixed pending inputs, exact IDs and genuine uncertainty in publication. Do not implement these abstract contracts just to retain this coordinator. |
| `rebuild_composition::{propose_rebuild,verify_full_readback,stage_cutover,commit_cutover}` | Builds in-memory plans/digests and accepts supplied readback. It does not create/fill a Qdrant generation or durably switch a route; no external caller found. | The [control owner](../../crates/search-control-redb/FUNCTIONS.md#restartable-generation-input-capture) captures actual build inputs; publication builds and switches the fresh generation. Delete the parallel plan/proof/staging chain. |
| Rebuild pin/reclaim helpers | Pin helpers mutate a process-local `PinRegistry`; `authorize_reclaim` returns a plan. Neither is called by the selected query, and the latter does no delete. | Preserve actual snapshot lifetime in the private view owner and exact cleanup in publication/bridge. Remove raw-pin, watermark and reclaimer proof/checkpoint plumbing with its exclusive callers. |

The real bridge's mutation/readback/query implementation remains reusable; it is not connected merely because
these modules mention Qdrant. Likewise the current public `--search-root`/`--serve-data-root` argv reaches the
earlier registry metadata route, not either the downstream DIRECT search or this query host. Replace actual
selection and owner calls together. The [CLI map](../eliot-search/FUNCTIONS.md#current-cli-selection-and-orphan-removal)
distinguishes active client code from removed orphan duplicates.

## Hidden query-time inventory defect

DIRECT query/page/continue/expand call `refresh_storage` before output. `StorageSecurityStatus::inspect` replays
the complete source event log, scans revisions for legacy GC, scans them again for protected headers and scans
preparation objects. A page can therefore do corpus/history-sized work merely to print storage metadata.

Remove this from normal startup/query/formatting. The root owner exposes configured storage mode cheaply; each
actual retained read verifies its object. File names or magic bytes do not establish that every object is
readable/authentic. Full verify/GC/archive/import are explicit expensive operations, not per-response credentials.

## Connected daemon

```text
startup:
  capture args_os/config once
  → explicit initialize or open current root
  → one process-lifetime root guard
  → construct current state/object/source/Qdrant owners
  → recover unfinished current work
  → install current view and concrete handlers
  → one framed local dispatcher
ingest:
  selected source → stable read → immutable ObjectRef
  → current SourceRecord/RevisionRef compare-set → one preparation outcome
  → Qdrant apply/readback → atomic CurrentPublishedView
query:
  typed request → pin current view + resolve scope + RequestContext
  → reserve finite candidate/window/output allowances
  → real Qdrant BM25 → pinned evidence row → retained raw/prepared read
  → one bounded terminal response → optional session QueryWindow
maintenance:
  stop daemon / acquire root exclusively
  → explicit local GC or completed archive/import
```

An arrow is not recovery. Current input/outcome comparison reconstructs lost wake-ups. Pending external effects
retain only finite unfinished state. Normal reads and pure transforms create no durable receipt.

## Surviving responsibilities

| Owner | Responsibility |
|---|---|
| invocation/root | one settings value, initialize/open, one real OS lock, bounded owned shutdown |
| source/object | stable acquisition; bytes durable before current-head commit |
| current state | short typed redb rows/transactions; no history replay per source operation |
| preparation | one materialization/unitization outcome, empty/gap/failure and stale-completion guard |
| Qdrant | one private runtime/client for publication and query |
| publication | fixed pending input, effects/readback, atomic view/evidence and finite cleanup |
| query/evidence | direct handler, pinned view/scope/context, composite lookup and retained read |
| session/window | one fixed-expiry final-order cursor with item indexes |
| protocol/CLI | framed local exchange, active Cancel and one owned-child lifetime |
| maintenance/import | stopped-root mark/sweep; completed archive; fresh destination/generation |

Responsibilities do not imply one crate each. Keep a boundary only for a real dependency/effect or repeated policy.

## Remove during cutover

- remaining argv interceptors and duplicate command implementations;
- registry/original-input replay and full aggregate snapshots for ordinary source work;
- duplicate object/preparation stores and compatibility formats;
- migration/sealed/secure-command applications and owner-slot/epoch/succession machinery;
- remaining wave features and optional old-package wiring with their actual consumers;
- CLI/provider-client endpoint/token/pairing/grant/capability/replay compatibility;
- wave4/test provider code after its live source/status consumers move;
- planner/executor/access/validator/projector proof systems and old rebuild/reclaimer composition;
- continuation/result-handle catalogues, full source fences and arbitrary range expansion;
- recursive storage-security/legacy-GC inventories from query/output;
- fixtures whose only subject is removed topology.

Unsupported old roots fail before writes and remain untouched. Do not expose a fixture mutation or shadowed
command as the replacement. Preserve original data and actual unpublished #598 overlap.

## Verification

After a Rust slice, run pinned Rust 1.98 check and strict Clippy for the actual changed production target and its
immediate consumers. `eliot-searchd --lib` does not cover its binary composition. Broad all-feature/all-test
matrices remain late; run failures and unavailable tooling are not successes.

The connected smoke uses default binaries and one real Qdrant route:

```text
initialize/open → ingest a real file → restart → publish/read back Qdrant
→ CLI Search → pinned evidence lookup → exact retained excerpt
```

Then exercise edit/rename, scope/IDF, old views, late writes, rebuild, cancellation, paging/expansion,
query-time cost, GC and fresh-destination restore. Help, self-test, compilation and constructors are not this
application-level evidence. Platform/recovery/relevance/scale claims require the corresponding execution.
