# Test and harness cutover

Tests preserve product behavior, not pre-release package layout, receipt graphs or disconnected applications.
Connect the real producer to the real consumer, run minimal pinned compiler/Clippy for changed production owners,
and delete tests whose only subject disappears. Broad regression and scale suites follow the connected product.

**Public baseline:** `c8e38b6fc9829f1cd38070773b1b3e5b21de18ad`  
**Latest inspected tree:** `5af8cde5416fa33e3f1a8f3c19367ed520eac9a4` (`R027` review, 2026-10-23).
The named Windows suites below were read completely; they were not executed or rewritten in this documentation pass.

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

Both suites use Windows-only direct `#[path]` source inclusion and synthetic migration inputs/allowances.
They do not exercise a connected current application. Their named classification is complete; transferring
useful behavior belongs to the receiving owner's implementation, not another audit pass.

In [control_migration_bootstrap_boundary.rs](../../bins/eliot-searchd/tests/control_migration_bootstrap_boundary.rs):

| Test | Disposition and receiving owner |
|---|---|
| `actual_retained_output_prevents_ancestor_replacement_and_target_clobber` | MOVE causal retained-parent/no-clobber behavior to the actual object writer. |
| `existing_target_is_preserved_and_never_reopened_as_original` | MOVE existing-target preservation to the object writer; no competing writer beside #598. |
| `locator_reservation_refusal_prevents_ancestor_storage_and_target_creation` | DELETE the synthetic migration allocation-allowance protocol. |
| `original_refusal_and_relative_or_escaped_output_fail_closed` | MOVE path containment to the actual output owner; DELETE the legacy original-context refusal shim. |
| `bootstrap_uses_one_target_before_inventory_and_cannot_run_legacy_import` | DELETE source-text ordering/old-import guards. Preserve the useful pre-effect budget and definite-not-performed versus possible-effect behavior in current control/publication owner tests; do not preserve `ATTEMPT_KEY`, old bootstrap rows or a generic operation journal merely to reproduce those strings. |

In [control_migration_inventory_owner_boundary.rs](../../bins/eliot-searchd/tests/control_migration_inventory_owner_boundary.rs):

| Test | Disposition and receiving owner |
|---|---|
| `ordered_windows_pages_cover_more_than_one_page_exactly_once` | MOVE bounded stable enumeration to stopped-root collection when its actual reader pages; do not retain the legacy page protocol. |
| `same_handle_bytes_are_exact_and_over_limit_never_read_or_truncated` | MOVE exact bounded reads to the retained object reader. The existing test observes a limit error, not whether every oversized-input byte avoided I/O; its name is not stronger execution evidence. |
| `missing_file_and_non_file_are_distinct_dispositions` | MOVE absent versus invalid-object distinction to the real reader. |
| `escaped_parent_is_refused_before_io` | MOVE containment to the current filesystem acquisition owner. |
| `original_context_refusal_stops_before_any_new_observation` | DELETE the synthetic legacy-context protocol. |
| `page_callback_drift_cannot_turn_disappearance_into_complete_absence` | MOVE drift/incomplete enumeration behavior to stopped-root collection; incomplete marks cannot authorize deletion. |
| `visit_retains_leaf_directory_and_blocks_replacement_between_pages` | MOVE retained-directory behavior only if the current walker spans such observations; otherwise DELETE the obsolete paging fixture. |
| `multi_link_object_is_a_gap_and_no_content_is_returned` | MOVE alias-safe identity and uncertain-object preservation to acquisition/collection. |
| `stale_unmapped_preparation_is_not_a_current_link_and_dangling_links_are_refused` | DELETE old DIRECT preparation-link protocol; preserve stale/dangling outcome refusal in the actual materializer/object reader. |
| `unknown_control_representation_is_incomplete_instead_of_silently_skipped` | MOVE unknown-metadata blocking of collection to the current collector; DELETE source-text assertions. |
| `empty_temporary_control_directory_has_a_metadata_row_before_recursion` | DELETE the old recursive control-directory representation/source-shape assertion. |

None is retained unchanged as a current-product test. Relevant owners are the
[object store](../../crates/search-source/search-revision-store/FUNCTIONS.md),
[safe reader](../../crates/search-source/search-safe-reader/FUNCTIONS.md),
[materializer](../../crates/search-prep/search-materializer/FUNCTIONS.md) and
[stopped-root collector](../../crates/search-runtime/search-retention/FUNCTIONS.md).
The [daemon caller map](../../bins/eliot-searchd/FUNCTIONS.md#inspected-callers-and-effects) records why removing
these test harnesses alone does not remove the still-callable migration route or its shared storage code.

## Remaining harness cuts

Still present:

- `wave1`–`wave6` and optional old-package wiring;
- CLI/provider-client loopback compatibility;
- wave4/test provider pairing/router/capability code;
- shared sealed/migration and query/rebuild/handle production modules;
- remaining migration/quarantine/old-command fixtures;
- lexical fixture-backed activation/profile authority.

Remove each owner with its last real caller. Do not replace the wave ladder with another acceptance profile or
package map. Default binaries must become the only product application.

R020 already removed `wave7-lifecycle`, `full-baseline` and the empty `search-retention` Cargo/workspace/lock
shell. Its remaining FUNCTIONS file owns maintenance behavior, not a crate to restore.

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
R027 changes this classification only; no Rust/test/manifest change or execution is claimed.
