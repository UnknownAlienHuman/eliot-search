# Test and harness cutover

Tests preserve product behavior, not pre-release package layout, receipt graphs or disconnected applications.
Connect the real producer to the real consumer, run minimal pinned compiler/Clippy for changed production owners,
and delete tests whose only subject disappears. Broad regression and scale suites follow the connected product.

**Public baseline:** `c8e38b6fc9829f1cd38070773b1b3e5b21de18ad`  
**Latest inspected tree:** `174f3ec758cb0ec5c38b780cd3de913be9120fdc` (`R019`, 2026-10-23)

## Classification rule

Every legacy test belongs to one of three buckets:

1. **Delete** when it parses source/manifests, freezes topology, requires an obsolete feature or proves an old
   state machine agrees with itself.
2. **Move the causal case** when it catches a real failure still relevant to a current owner. Rewrite it against
   that owner; do not retain the old command, receipt or transport to keep the fixture alive.
3. **Retain unchanged** only when it already executes selected current behavior.

A subprocess, native call, real Qdrant binary or large fixture does not by itself make an obsolete architecture a
product requirement.

## Removed proof systems

### R015–R017

Removed source-text topology tests, restore/retention/purge proof suites, eight standalone snapshot/sealed Cargo
applications, private snapshot/control/local-BM25 code, daemon `restore_composition` and generic
retention/purge/sweep/restore source.

Preserved causal requirements: complete mark before deletion; foreign and sole-copy preservation; archive marker
last; fresh destination/Qdrant identity; no copied nonce continuity; bounded stable acquisition; no-clobber
objects; link/reparse refusal; exact coordinates and truthful gaps.

### R018 — disconnected query/provider/rebuild fixtures

Removed nine process fixtures that explicitly did not execute the selected product or existed only for rejected
mechanisms:

- in-memory indexed query oracle and plan/access/executor/validator/projector chain;
- manually joined DIRECT plus duplicate Qdrant REST half;
- old rebuild/pin/reclaimer receipt composition;
- stale `QualifiedGate` live lane and indexed-unavailable gate test;
- pairing/vault/token loopback and provider envelope/replay proxy;
- fixed eleven-recipe matrix;
- random result-handle/full-source-fence/continuation lifecycle.

Causal cases moved to current owners:

- real selected scope, base IDF population and exact readback;
- unknown mutation resolution and lost-index rebuild into a fresh generation;
- old-view guard blocking cleanup and partial readback blocking cutover;
- wrong point versus missing evidence versus request cancellation;
- known empty scope without wildcard widening and non-exhaustive top-k truthfulness;
- fixed window expiry, same-session item locators and no cursor advance on partial output;
- active Cancel/EOF and bounded owned-child cleanup;
- direct literal/exact/navigation/comparison behavior without a recipe registry.

### R019 — daemon loopback production owner

After R018 removed all process users, caller inspection found daemon `legacy-loopback-harness` reachable only
through `entry.rs`:

```text
entry interceptor
→ authenticated_proxy
→ endpoint pairing/TCP server
→ proxy child and exchange
→ token/envelope/replay protocol
```

No current integration test names `endpoint` or imports these modules. R019 removes that feature, interceptor and
36 production/source-test files. It also removes `legacy-loopback-harness` from provider-composition cfg clauses;
wave4/test provider code remains for a later connected-query cut.

Do not move TCP/token/pairing tests to the new transport. The selected baseline is trusted owned-child local IPC.
Move only general behavior:

- complete response before visibility;
- finite framing and aggregate output bounds;
- cancellation/EOF during active work;
- every child exit path closes/terminates/reaps the owned process;
- transport failure never fabricates mutation rollback or a success receipt.

CLI/provider-client loopback compatibility remains separately and must be removed with its own callers and Cargo
feature. R019 does not claim the new framed protocol is implemented.

## Mixed migration tests retained for behavior extraction

`control_migration_bootstrap_boundary.rs` and `control_migration_inventory_owner_boundary.rs` mix real
Windows/filesystem cases with source-text assertions. Move these causal cases before deleting the harness:

- no-clobber creation and retained parent-directory identity;
- bounded exact reads, missing versus non-file and path-escape refusal;
- membership drift detection between pages;
- hard-link/alias refusal;
- incomplete/unknown metadata remains a gap rather than silent absence.

## Remaining harness cuts

Still present:

- `wave1`–`wave7`/`full-baseline` and optional old-package wiring;
- empty `search-retention` Cargo/workspace/lock shell;
- CLI/provider-client loopback compatibility;
- wave4/test provider pairing/router/capability code;
- shared sealed/migration and query/rebuild/handle production modules;
- remaining migration/quarantine/old-command fixtures;
- lexical fixture-backed activation/profile authority.

Remove each owner with its last real caller. Do not replace the wave ladder with another acceptance profile or
package map. Default binaries must become the only product application.

## Target portfolio

### Fast owner tests

Keep deterministic tests for:

- typed redb compare/set and restart readback;
- immutable object create/reuse/no-clobber/final verification;
- preparation empty/gap states and coordinate maps;
- stable retrieval identity and published evidence binding;
- real Qdrant conversion, mutation/readback and publication recovery;
- view-guard cleanup and window commit after complete terminal output;
- framed protocol bounds, cancellation and partial I/O.

They call current functions; they do not inspect repository text or issue state.

### One connected smoke

```text
initialize/open current root
→ ingest one real file
→ restart
→ publish/read back native BM25 state
→ CLI Search
→ pinned evidence lookup
→ exact retained excerpt
```

A constructor, process oracle, duplicate REST client, source-shape assertion or self-test string is not this
smoke.

### Focused connected regressions

Edit/rename; explicit/empty/unknown scope; correct IDF population; old pinned view; unknown Qdrant write;
lost-index rebuild; cancellation/deadline; failed terminal output; bounded mapped expansion; unsupported/binary/
missing evidence; exact no-match; stopped-root GC; archive/import.

Optional PDF/model/overlay/MCP tests exist only after a real producer, consumer and selected runtime.

## Execution policy

For each Rust cutover:

1. identify the old owner and last callers;
2. classify tests as delete, move causal behavior or retain;
3. connect the replacement and delete the old owner/fixtures in the same slice where possible;
4. run Rust 1.98 `check` and strict Clippy for changed production owners and immediate consumers;
5. defer broad all-feature/all-test, native qualification and scale matrices until the connected product exists.

R015–R019 have no new compiler/runtime claim. The available Actions workflow is manual-only and no dispatch tool
or local Cargo runtime was available for these SHAs. Static caller inspection is not a build result.
