# eliot-searchd — one application, one owner chain

Follow the root [architecture](../../docs/architecture/README.md) and
[implementation](../../docs/implementation/README.md). Existing command/package/receipt boundaries are not
compatibility requirements.

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

Each captures arguments/config and may shadow later handlers. The CLI's advertised persistent child can therefore
reach a different implementation or refusal than its help describes. Historical wave features compile more
disconnected planner/access systems; they do not select one product.

Cleanup already performed:

- R016 removed standalone snapshot daemon/CLI and six sealed Cargo test applications;
- R017 removed daemon `restore_composition` and generic retention/purge/sweep/restore implementation;
- R018 removed process proofs for the in-memory indexed stack, manually joined DIRECT/Qdrant halves, qualification,
  pairing/token provider, recipes, rebuild/reclaimer and random handles;
- R019 removed the daemon `legacy-loopback-harness`, `authenticated_proxy`, endpoint pairing server and its private
  proxy child/exchange implementation. The primary daemon no longer accepts TCP port/token-file startup.

The empty `search-retention` package and `wave7-lifecycle` manifest records remain only for a separate Cargo/lock
cut. CLI/provider-client loopback compatibility is a separate remaining layer. Shared `sealed_*`, migration,
provider, query, rebuild and compatibility production modules still remain and must be removed with actual callers.

Representative current islands:

- source admission restores/requalifies registry history yet drops acquired bytes;
- source sync still returns `SOURCE_ROOT_SYNC_EFFECTS_UNAVAILABLE`;
- preparation uses another DIRECT store/profile/receipt family;
- indexed query proof modules are not connected to the real default handler;
- DIRECT query still uses continuation plus random result-handle catalogues and the old line protocol;
- provider composition still contains wave4/test pairing/grant/envelope code after the daemon loopback owner and
  its process proofs disappeared;
- wave features still aggregate optional packages instead of constructing one application.

Do not fix reachability by adding another dispatcher, facade or compatibility version. Select the current path,
move useful behavior, and delete the old owner with its actual callers.

## Hidden query-time inventory defect

The inspected DIRECT query, page, continue and expansion commands call `refresh_storage` before output.
That calls `StorageSecurityStatus::inspect`, which:

1. reads and verifies the complete append-only source event log;
2. scans the entire revision tree for the legacy GC inventory;
3. scans the revision tree again for protected-object headers;
4. scans preparation refs/objects.

Thus one page can perform work proportional to all historical events and stored objects merely to print storage
metadata. File suffixes and magic bytes do not prove that every object is readable/authentic, and this scan is not
request authorization.

Remove it from normal startup/query/formatting. The root owner exposes configured storage mode cheaply; each
actual retained read verifies its object. Full verify/GC/archive/import are explicit expensive operations, not
per-response credentials.

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
  selected source
  → stable read
  → immutable ObjectRef
  → current SourceRecord/RevisionRef compare-set
  → one preparation outcome
  → Qdrant apply/readback
  → atomic CurrentPublishedView
query:
  typed request
  → pin current view + resolve scope + RequestContext
  → real Qdrant BM25
  → pinned evidence row
  → retained raw/prepared read
  → one bounded terminal response
  → optional session QueryWindow
maintenance:
  stop daemon / acquire root exclusively
  → explicit local GC or completed archive/import
```

An arrow is not recovery. Current input/outcome comparison reconstructs lost wake-ups. Pending external effects
retain only finite unfinished state. Normal reads and pure transforms create no durable receipt.

## Surviving responsibilities

| Owner | Responsibility |
|---|---|
| invocation/root | one command/settings value, initialize/open, one real OS lock, bounded owned shutdown |
| source/object | stable acquisition and one immutable writer/reader; bytes durable before current-head commit |
| current state | short typed redb rows/transactions; no operation-history restore per command |
| preparation | one materialization/unitization result, explicit empty/gap/failure and stale-completion guard |
| Qdrant | one private runtime/client used by publication and query |
| publication | fixed pending input, effects/readback, atomic view/evidence commit and finite cleanup |
| query/evidence | direct handler, pinned view/scope/context, exact composite lookup and retained read |
| session/window | one fixed-expiry final-order cursor with item indexes; no source fences or random handles |
| protocol/CLI | one framed typed local exchange with active Cancel path and one owned-child lifetime |
| maintenance/import | stopped-root typed mark/sweep; archive marker last; fresh destination/generation |

Responsibilities do not imply one crate each. Concrete internal calls are preferred; retain a boundary only for
a real dependency/effect or repeated policy.

## Remove during cutover

- remaining argv interceptor chain and duplicate command/help implementations;
- registry/original-input replay and full aggregate snapshots for ordinary source work;
- duplicate object/preparation stores and compatibility formats;
- migration/sealed/secure-command applications and owner-slot/epoch/succession machinery;
- wave feature ladder, empty `search-retention` shell and optional old-package wiring;
- CLI/provider-client endpoint/token/pairing/grant/capability/replay compatibility;
- wave4/test provider pairing/router/capability code when direct typed handlers replace it;
- planner/executor/access/validator/projector proof operating systems;
- rebuild/reclaimer composition after current publication owns rebuild and cleanup;
- continuation/result-handle catalogues, full source fences and arbitrary range expansion;
- recursive storage-security/legacy-GC scans from query/output;
- source-shape/process fixtures whose only subject is removed topology.

Unsupported old roots fail before writes and remain untouched. Do not expose a fixture mutation or shadowed
command as the replacement.

## Verification

For each Rust slice, run pinned Rust 1.98 check and strict Clippy for changed production owners and immediate
consumers. Broad all-feature/all-test matrices remain late.

The connected smoke uses default binaries and one real Qdrant route:

```text
initialize/open
→ ingest one real file
→ restart
→ publish/read back Qdrant
→ CLI Search
→ pinned evidence lookup
→ exact retained excerpt
```

Then exercise edit/rename, explicit scope/IDF, old pinned view, unknown/late write, lost-index rebuild,
cancellation, paging/expansion, no query-time storage inventory, stopped-root GC and fresh-destination restore.
Native/platform/recovery/relevance/scale claims require actual execution, not constructor, process-oracle or
manual-two-halves tests.
