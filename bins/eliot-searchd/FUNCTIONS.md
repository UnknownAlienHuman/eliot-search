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
→ optional proxy
→ DIRECT session
→ secure command stack
→ final shell
```

Each captures arguments/config and may shadow later handlers. The CLI's advertised persistent child can therefore
reach a different implementation or refusal than its help describes. Enabling historical wave features compiles
more disconnected planner/access/restore systems; it does not select one product.

R016 removed the explicit snapshot daemon/CLI and six sealed Cargo test applications together with the exclusive
snapshot control store, snapshot format and in-memory BM25 implementation. They were independent products, not
layers of `entry.rs`. Shared `sealed_*`, migration, restore and compatibility modules still remain in the primary
package and must be removed only with their actual callers.

Representative current islands:

- source admission restores/requalifies registry history yet drops acquired bytes;
- source sync still returns `SOURCE_ROOT_SYNC_EFFECTS_UNAVAILABLE`;
- preparation uses another DIRECT store/profile/receipt family;
- indexed query proof stacks are wired to fixtures/test ports rather than the real Qdrant/evidence path;
- DIRECT query uses continuation plus random result-handle catalogues and the old line protocol;
- `wave7-lifecycle` can still compile a same-installation restore/purge/receipt state machine even though its
  exclusive proof fixtures and standalone targets have been removed.

Do not fix reachability by adding another dispatcher, facade or compatibility version. Select the current path,
move useful behavior, and delete the old owner with its actual callers.

## Hidden query-time inventory defect

The inspected DIRECT query, page, continue and expansion commands call `refresh_storage` before output.
That calls `StorageSecurityStatus::inspect`, which:

1. reads and verifies the complete append-only source event log;
2. scans the entire revision tree for the legacy GC inventory;
3. scans the revision tree again for protected-object headers;
4. scans preparation refs/objects.

Thus an interactive page can perform work proportional to all historical events and stored objects merely to
print `storage_security`/`encrypted_at_rest`. File suffixes and magic bytes do not prove that every object is
readable/authentic, and this scan is not request authorization.

Remove it from normal startup/query/formatting. The root owner exposes the selected storage mode cheaply; each
actual retained read verifies its object through the real format. A separate explicit verify/maintenance command
may run a full inventory and report exactly what it checked. Do not cache that report as a capability token or
replay it per response.

## Connected daemon

```text
startup:
  capture args_os/config once
  → explicit initialize or open current root
  → one process-lifetime root guard
  → construct current state/object/source/Qdrant owners
  → recover unfinished current work
  → install current view and concrete handlers
  → one framed dispatcher
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
| protocol/CLI | one framed typed exchange with active Cancel path and one owned-child lifetime |
| maintenance/import | stopped-root typed mark/sweep; archive marker last; fresh destination/generation |

Responsibilities do not imply one crate each. Concrete internal calls are preferred; retain a boundary only for
a real dependency/effect or repeated policy.

## Availability and diagnostics

Availability comes from constructed handlers and dependencies. A descriptor, receipt, qualification report or
method name cannot advertise an absent path. Health/status must distinguish:

- configured storage/backend mode;
- currently constructed handlers;
- current/published source state and explicit gaps;
- last operation's real failure;
- an explicit deep verification result when the user requested one.

It must not recursively scan the corpus/object store on every request, infer freshness from watcher silence, or
claim complete encryption/integrity from object names. Full verify, GC, backup and restore are explicit expensive
operations with their own output and cancellation.

## Remove during cutover

- argv interceptor chain and duplicate command/help implementations;
- registry/original-input replay and full aggregate snapshots for ordinary source work;
- duplicate object/preparation stores and compatibility formats;
- migration/sealed/secure-command applications and owner-slot/epoch/succession machinery still shared by the
  primary package after the standalone R016 targets were deleted;
- wave feature ladder and optional old-package wiring;
- provider pairing/grant/capability/receipt stacks;
- planner/executor/access/validator/projector proof operating systems after direct handlers replace them;
- process-test indexed oracle/manual REST halves;
- continuation/result-handle catalogues, full source fences and arbitrary range expansion;
- recursive storage-security/legacy-GC scans from query/output;
- generic reclaimer/pin/retention/purge/restore platforms, including daemon `restore_composition`;
- source-shape/process fixtures whose only subject is removed topology.

Unsupported old roots fail before writes and remain untouched. Do not expose a fixture mutation or shadowed
command as the replacement.

## Verification

For each Rust slice, run pinned Rust 1.98 check and strict Clippy for changed production owners and immediate
real consumers. Broad all-feature/all-test matrices remain late.

The connected smoke is the default binaries and one real Qdrant route:

```text
initialize/open
→ ingest one real file
→ restart
→ publish/read back Qdrant
→ CLI Search
→ pinned evidence lookup
→ exact retained excerpt
```

Then exercise edit/rename, explicit scope/IDF, old pinned view, lost wake-up, pending publication recovery,
lost-index rebuild, cancellation, paging/expansion, no query-time storage inventory, stopped-root GC and fresh-
destination restore. Native/platform/recovery/relevance/scale claims require actual execution, not constructor or
source-shape tests.
