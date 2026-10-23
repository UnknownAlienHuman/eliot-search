# Test and harness cutover

Tests preserve product behavior, not pre-release package layout, receipt graphs or disconnected applications.
During refactoring, connect the real producer to the real consumer, run minimal pinned compiler/Clippy for the
changed production slice, and delete tests whose only subject disappears. Broad regression and scale suites
follow the connected application.

**Public baseline:** `c8e38b6fc9829f1cd38070773b1b3e5b21de18ad`  
**Latest inspected tree:** `151456672ea4ec8773ed65c5017cb63989912d30` (`R018`, 2026-10-23)

## Classification rule

Every legacy test belongs to one of three buckets:

1. **Delete with no replacement** when it parses Rust/Cargo/Markdown text, freezes filenames/type placement,
   requires an obsolete feature or only proves an old state machine agrees with itself.
2. **Move the causal case** when it catches a real failure that still applies to the new owner. Rewrite the case
   against the current API; do not retain the old command, receipt or fixture to keep the test alive.
3. **Retain unchanged** only when it already executes current product behavior through the selected owner.

A subprocess, native call or large fixture does not make an obsolete architecture a product requirement.

## R015-R017: remove topology and lifecycle proof systems

R015 removed source-text topology tests and exclusive restore/retention/purge suites. R016 removed eight
standalone snapshot/sealed Cargo applications and their private snapshot/control/local-BM25 source. R017 removed
the feature-gated daemon restore implementation and generic retention/purge/sweep/restore source, leaving only an
empty package shell for the later manifest/lock cut.

The useful causal requirements survived without those systems: complete marks before deletion, foreign/sole-copy
preservation, archive marker last, fresh destination/Qdrant identities, no copied nonce continuity, stable
bounded acquisition, no-clobber immutable objects, link/reparse refusal, exact coordinates and truthful gaps.

## R018: remove disconnected process proof stacks

The following fixtures explicitly did not execute the selected product or existed only for mechanisms the target
removes. R018 deletes them rather than preserving their architecture through test pressure:

- `indexed_query_process.rs` — in-memory Qdrant oracle plus canonical plan, access permits, leg tickets, candidate
  readback proofs and projector; its header says it does not drive the served daemon;
- `product_spine_process.rs` — real DIRECT commands manually joined to a second hand-written Qdrant REST client;
  its header says there is no single CLI indexed query;
- `rebuild_process.rs` — old rebuild/reclaimer composition over the in-memory oracle, pin/receipt machinery and
  an ordinary-reclaim proof system;
- `qdrant_grpc_parity_process.rs` — stale live-qualification lane that constructs `QualifiedGate`, already removed
  from the selected bridge;
- `indexed_provider_process.rs` — proves indexed query is unavailable behind the old qualification gate;
- `pairing_process.rs` — token/vault/pairing handshake system for loopback TCP; trusted owned-child stdio needs no
  product pairing or frame MAC;
- `provider_process.rs` — canonical envelopes, grants/pairing/replay and endpoint/token files over the obsolete
  loopback proxy;
- `recipes_process.rs` — freezes eleven recipe IDs and support matrices while the target uses direct operations;
- `live_handles_process.rs` — random result handles, whole-source fences and continuation catalogues that are
  replaced by one session-owned `QueryWindow`.

These nine files total 243,346 source bytes. Their deletion does not implement the replacement and does not erase
the following causal requirements:

### Qdrant/publication cases to move

- selected scope and base IDF population exclude foreign points without widening;
- exact point/schema readback follows real effects;
- unknown mutation outcome is resolved from actual state, never a fabricated receipt;
- lost collection rebuilds from retained/current control truth into a fresh generation;
- complete readback precedes view installation;
- an old pinned view blocks cleanup until its real guard releases;
- partial readback or stale/foreign generation never cuts over.

These belong to the real bridge/publication path, not an in-memory oracle or duplicate REST client.

### Query/window cases to move

- wrong/foreign point and genuinely missing retained evidence are distinct;
- empty known scope is valid; unknown scope fails; bounded top-k does not claim exhaustive coverage;
- page admission occurs before excessive allocation/source reads;
- one fixed-expiry window owns final refs and item indexes;
- independent session/restart cannot reuse a locator;
- mutation of current source does not invalidate evidence retained by an old pinned view;
- expansion is bounded relative context in the emitted representation;
- partial/cancelled terminal output advances no cursor.

### Protocol/client cases to move

- one complete capped frame is validated before cards/locators become visible;
- Cancel/EOF remains observable during active work;
- every post-spawn exit closes, terminates if needed and reaps only the owned child;
- replay/sequence/token-envelope machinery is not retained for trusted child stdio;
- unavailable handlers are represented by construction, not qualification credentials.

### Product task cases to move

Literal search, exact absence, provenance, expansion, orientation and comparison remain user behaviors. They do
not require a fixed recipe registry, support matrix, grant or receipt family. Test the direct handler that actually
implements each task.

## Mixed migration tests retained only for behavior extraction

`control_migration_bootstrap_boundary.rs` and `control_migration_inventory_owner_boundary.rs` contain real
Windows/filesystem cases mixed with source-text assertions. Do not delete their useful cases blindly:

- no-clobber target creation and retained parent-directory identity;
- bounded exact reads, missing versus non-file distinction and path-escape refusal;
- directory membership drift detection between pages;
- hard-link/alias refusal;
- incomplete/unknown metadata remains a gap rather than silent absence.

Move those cases to current root/object/source/archive owners during their cutover. Delete the remaining
`include_str!`/marker-order assertions and legacy migration harness at that time.

## Remaining harness cuts

The explicit snapshot/sealed targets, old lifecycle implementation and nine disconnected proof fixtures are gone.
The current package still contains:

- `wave1` through `wave7`/`full-baseline` feature composition and optional old-package wiring;
- an empty `search-retention` Cargo/workspace/lock shell pending coordinated removal;
- shared `sealed_*`, migration, pairing/provider/handle/query proof and compatibility/interceptor production code;
- process fixtures for migration, quarantine and remaining old command surfaces;
- lexical fixture-backed activation/profile authority.

Remove each owner with its last real caller and move only its causal behavior. Do not replace the wave ladder
with another test profile, package map or acceptance feature. The default binaries must become the only product
application.

## Target test portfolio

### Fast owner tests

Keep small deterministic tests next to the current implementation for:

- typed redb compare/set and restart readback;
- immutable object create/reuse/no-clobber/final verification;
- preparation empty/gap states, coordinate maps and structure-first unit boundaries;
- stable retrieval identity and published evidence binding;
- Qdrant request/response conversion and full versus narrow mutation semantics;
- publication cursor transitions, late-writer handling and view-guard cleanup eligibility;
- request/window bounds and commit after complete terminal output;
- framed protocol bounds, cancellation and partial I/O.

These tests call current functions. They do not inspect repository text, issue state or module placement.

### One connected smoke

The primary acceptance path uses default binaries and the real selected Qdrant client:

```text
initialize/open current root
→ ingest one real file
→ restart
→ publish and read back native BM25 state
→ CLI Search
→ pinned evidence lookup
→ exact retained excerpt
```

A constructor test, process oracle, manual REST half, source-shape assertion or self-test string is not this
smoke.

### Focused connected regressions

After the path exists, add only causal cases: edit/rename; explicit/empty/unknown scope; correct IDF population;
old pinned view; unknown Qdrant mutation outcome; lost-index rebuild; cancellation/deadline; failed page output;
bounded mapped expansion; unsupported/binary/missing evidence; exact no-match; stopped-root GC; archive/import.

Optional PDF/model/overlay/MCP tests exist only after that feature has a real producer, consumer and selected
runtime.

## Execution policy

For each Rust cutover:

1. identify the old owner/API and its last real callers;
2. classify its tests as delete, move causal behavior or retain;
3. connect the new producer/consumer and delete the old implementation and exclusive fixtures in the same slice;
4. run Rust 1.98 `check` and strict Clippy for changed production owners and immediate consumers;
5. defer broad workspace/all-feature/all-test, native qualification, scale and long recovery matrices until the
   connected product exists.

Test volume is not evidence of a useful product boundary. Do not preserve a removed subsystem merely because
its fixtures are large.
