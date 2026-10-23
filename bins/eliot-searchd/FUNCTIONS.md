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
