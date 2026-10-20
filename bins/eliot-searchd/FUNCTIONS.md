# eliot-searchd — source routing and replacement map

Follow the [implementation guide](../../docs/implementation/README.md). Existing command/package/receipt
boundaries are not compatibility requirements.

## Root cause: several applications share one binary

The production entrypoint does not parse one command and construct one application. It calls nine interceptors
in sequence; each independently recaptures argv/config and may claim the command:

```text
legacy-effect refusal
→ direct registry/journal
→ native root lifecycle
→ offline migration
→ legacy preparation
→ optional loopback
→ read-only DIRECT session
→ second persistent command stack
→ shell/stdio/one-shot application
```

This is behaviorally unsafe even where each component is locally careful. Command ownership depends on order,
later handlers can be unreachable and help text can describe a different implementation from the one that
receives the argv.

### Confirmed shadowing

`direct_registry_command::is_direct_command` recognizes:

```text
register/unregister/source-roots/sync-source-roots
admit-source-v2 and DEK commands
health/verify/list/search/read-revision
serve-data-root
```

It runs before both `public_runtime_service` and `secure_commands::maybe_run`. Its non-source/non-DEK path does
not dispatch by the read-looking command name: it restores/requalifies the complete registry, republishes a
control snapshot and emits `DirectOutput::metadata`, including `ready:false`,
`source_body_serving:false` and `source_read_refusal`. Thus:

- `--serve-data-root` cannot reach the later session which emits `data_root_ready`;
- `--search-root` cannot reach the later source search implementation;
- `--list-sources`, `--read-revision`, health/verify and sync names are similarly captured as registry work;
- the later secure registration/sync/read commands are duplicate unreachable routes for the same argv.

The primary CLI's interactive client starts `eliot-searchd --serve-data-root ROOT` and waits for
`data_root_ready`, so its advertised persistent session cannot start against this entrypoint.

### Other reachable islands

- Native root initialize/inspect/owner-check uses the large installation/owner-slot lifecycle.
- `--prepare-root` and `--prepare-revision` operate a separate legacy `DirectStore`.
- Three control-migration commands stage old mapping/bootstrap/source artifacts offline.
- The final shell application provides one-shot scan, stdio health/status and a literal self-test claim.
- Early refusal owns index/directory/retire/repair/GC command names.

None joins one fresh root, one source body, one immutable object, Qdrant publication and source-backed query.

## Feature-ladder defect

`bins/eliot-searchd/Cargo.toml` defaults to empty `wave1-shell`. Qdrant and query packages are omitted unless
historical wave features are manually enabled. `full-baseline` includes Qdrant/query but also disconnected
access/planner/executor/validator/projector, multiple handle/continuation systems, overlay, proof/comparison,
reclaimer, evaluation and generic retention. Enabling it compiles more source; it does not select one runtime
or make dead composition reachable.

Cut over to direct required dependencies for the current application and delete the wave features with their
last cfg/caller. Do not make `full-baseline` the default as a substitute for integration.

## Connected target

```text
startup:
  one args_os/config capture
  → explicit initialize or open current root
  → one process-lifetime guard
  → current state/object/source/Qdrant owners
  → current view + handlers
  → one dispatcher

ingest:
  selected source
  → stable read
  → immutable ObjectRef
  → current SourceRecord/RevisionRef compare-set
  → one MaterializationProduct
  → Qdrant apply/readback
  → CurrentPublishedView
query:
  SearchRequest
  → pin current view + resolve scope + RequestContext
  → Qdrant BM25
  → ViewManifestEntry
  → retained object/range verification
  → EvidenceCard[]
  → optional QueryWindow
```

## Owners which survive

| Owner | Responsibility |
|---|---|
| invocation/root | one command/settings value, initialize/open, one actual OS lock, orderly shutdown |
| source/object | stable acquisition and one immutable writer/reader; bytes before source-head commit |
| current state | short typed redb rows/transactions; no operation archive or full restore per command |
| preparation | one materialization and structure-first units; unsupported transforms become gaps |
| Qdrant | one private reusable client/runtime for publication and query |
| publication | pending artifact/cursor, external effect/readback, current view and finite cleanup |
| query/evidence | direct handlers, pinned view/scope/context, manifest lookup and exact retained read |
| window | one temporary final-order cursor/view guard and bounded item locator |
| protocol | one framed typed exchange; actual constructed handlers determine availability |

## Remove during cutover

- argv interceptor chain and duplicate command/help implementations;
- full registry replay/original inputs/full control snapshot for ordinary source operations;
- old installation owner slots/epochs/succession and duplicate locks after one guard replaces them;
- legacy preparation/object store beside the selected object owner;
- migration, sealed, secure-command and DIRECT compatibility applications for unsupported pre-release roots;
- wave feature ladder and dead-code composition facades;
- provider pairing/grant/capability/receipt stacks;
- planner/executor/access/validator/projector operating systems when direct handlers replace them;
- process-test indexed backend and caller-shaped authority;
- four continuation/handle catalogues, random source handles and catalogue-wide rescans;
- generic reclaimer/pin/retention/restore platforms after publication cleanup and object GC own the work.

Unsupported old roots fail before writes and remain untouched. Do not move a shadowed command into another
facade or expose a test-only mutation implementation as the replacement.

## Verification

After code, exercise the actual default binaries: help/version without root effects; initialize/open; ingest
and retain one file; restart; publish/read back Qdrant; Search from CLI; reopen exact excerpt; edit/rename;
source scope/IDF; old pinned view; pending publication recovery; lost-index rebuild; cancellation; paging and
bounded expansion. Public help/status must match only the handlers actually constructed.

Run minimal Rust 1.98 check and strict Clippy for changed production owners and immediate consumers. Native,
live-Qdrant and recovery claims require actual execution; feature presence, constructors and old receipts do
not establish them.
