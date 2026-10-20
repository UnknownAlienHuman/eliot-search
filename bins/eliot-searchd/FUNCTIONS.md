# eliot-searchd — source routing and replacement map

Follow the [implementation guide](../../docs/implementation/README.md) and
[test cutover](../../docs/implementation/TEST_CUTOVER.md). Existing command/package/receipt boundaries are not
compatibility requirements.

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
receives argv.

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
control snapshot and emits metadata including `ready:false`, `source_body_serving:false` and
`source_read_refusal`. Thus later serve/search/list/read implementations are shadowed by registry work.

The primary CLI's interactive client starts `eliot-searchd --serve-data-root ROOT` and waits for
`data_root_ready`, so its advertised persistent session cannot start against this entrypoint.

### Other reachable islands

- native initialize/inspect/owner-check uses the large installation/owner-slot lifecycle;
- `--prepare-root` and `--prepare-revision` use another legacy store;
- control-migration commands stage old mapping/bootstrap/source artifacts offline;
- the final shell provides one-shot scan, stdio diagnostics and a literal self-test claim;
- early refusal owns index/directory/retire/repair/GC names.

None joins one fresh root, one source body, one immutable object, Qdrant publication and source-backed query.

## Feature and test topology defect

`Cargo.toml` defaults to `wave1-shell`. Qdrant/query packages appear only through historical wave features.
`full-baseline` also enables disconnected access/planner/executor/validator/projector, multiple handle/
continuation systems, overlay, proof/comparison, reclaimer, evaluation and generic retention. Enabling more
features compiles more source; it does not select one application.

The manifest additionally declares seven harness-only test binaries for snapshot/sealed applications. The
main test directory contains 76 Rust files: 33 `*_module_ownership.rs` and 28 `*_process.rs`.

Representative ownership tests read Rust files as text and assert exact module names, marker strings, byte
ceilings, old lifecycle states and even retained test-function names. They are source-shape policy, not
behavior. `indexed_query_process.rs` explicitly drives a process-test in-memory oracle rather than the served
daemon. `product_spine_process.rs` explicitly joins legacy DIRECT and manually implemented Qdrant REST halves,
not one CLI operation through the real bridge.

Cut over direct required dependencies and delete wave features, harness-only binaries, source-shape tests and
process proofs with their old owners. Move only still-valid causal cases into current owner/product tests; do
not make `full-baseline` default or preserve a subsystem merely because it has many fixtures.

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

## Test migration

Delete immediately with the old source owner:

- module/file/function-name and line/byte-ceiling assertions;
- source-token/forbidden-string checks which duplicate compiler/API review;
- fixtures whose sole subject is owner slots, migration, quarantine, pairing, grants, handles,
  continuation, purge, overlay or other retired state machines;
- process tests which connect test doubles or manually join independent halves;
- seven harness-only snapshot/sealed targets and their exclusive source after the last unique behavior moves;
- wave-feature acceptance and frozen recipe/profile/schema/golden tests for removed APIs.

Move useful behavior into the current path. Existing DIRECT tests contain cases worth preserving, including
cross-boundary matches, exact line coordinates, binary-content gaps, historical retained readback, ABA content
reuse, overlapping matches and truthful output-limit completion. Express these through the new source/object/
preparation/query owners rather than old commands, credential cleanup and stores.

The main product smoke must execute default binaries and one real Qdrant route:

```text
initialize/open
→ ingest one real file
→ restart
→ publish/read back Qdrant
→ CLI Search
→ manifest lookup
→ exact retained excerpt
```

A passing constructor, oracle, source-shape test, manual REST half or self-test string is not completion.

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
- generic reclaimer/pin/retention/restore platforms after publication cleanup and object GC own the work;
- exclusive source-shape/process/harness fixtures for every removed family.

Unsupported old roots fail before writes and remain untouched. Do not move a shadowed command into another
facade or expose a test-only mutation implementation as the replacement.

## Verification

During each Rust slice, run minimal Rust 1.98 check and strict Clippy for changed production owners and their
immediate real consumers, plus only focused causal cases. Broad workspace/all-feature/all-test matrices are
late integration/release work.

After the path connects, exercise the actual default binaries: help/version without root effects;
initialize/open; ingest and retain one file; restart; publish/read back Qdrant; Search from CLI; reopen exact
excerpt; edit/rename; scope/IDF; old pinned view; pending publication recovery; lost-index rebuild;
cancellation; paging and bounded expansion. Native/live-Qdrant/recovery claims require actual execution.
