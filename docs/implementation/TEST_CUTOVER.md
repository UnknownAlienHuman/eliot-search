# Test and harness cutover

Tests preserve product behavior, not pre-release package layout, receipt graphs or disconnected applications.
Connect the real producer to the real consumer, run minimal Rust 1.98 compiler/Clippy for changed production
owners, and delete tests whose only subject disappears. Broad regression, native and scale suites follow the
connected product.

**Historical source baseline:** `c8e38b6fc9829f1cd38070773b1b3e5b21de18ad`
**Current documentation checkpoint:** R032 over `da382b66e59d0b44e758bad9b22e2f5703a4e8c2`.

## Classification rule

Every legacy test has one disposition:

1. **Delete** when it freezes source layout, package topology, receipts, obsolete features or a removed state
   machine.
2. **Move causal behavior** when it exposes a failure still relevant to a current owner. Rewrite against that
   owner; do not preserve the old command/protocol merely to keep the fixture alive.
3. **Retain unchanged** only when it already executes selected current behavior through a real consumer.

A subprocess, native call, real Qdrant binary or large fixture does not make an obsolete architecture current.

## Completed removals

R015–R020 removed source-shape proof suites, standalone snapshot/sealed applications, generic
retention/restore/purge code, disconnected query/provider/rebuild process proofs, the daemon TCP/token proxy,
the empty retention package and `wave7-lifecycle`/`full-baseline` aliases. R028 removed five unreferenced CLI
source copies. R030 removed the unused `search-eval` package and its audit/receipt/verdict APIs.

Useful requirements survived with receiving owners: no-clobber object publication, exact retained reads,
conservative incomplete inventories, fresh-destination restore, real Qdrant scope/readback, old-view lifetime,
bounded framed output, active cancellation and owned-child cleanup.

## Mixed migration suites — complete disposition

The two Windows suites use direct `#[path]` source inclusion and synthetic migration inputs. They do not execute
the target application. All sixteen tests have receiving-owner or delete dispositions; do not audit them again.

### `control_migration_bootstrap_boundary.rs`

| Test | Disposition |
|---|---|
| `actual_retained_output_prevents_ancestor_replacement_and_target_clobber` | MOVE retained-parent/no-clobber behavior to the immutable object writer. |
| `existing_target_is_preserved_and_never_reopened_as_original` | MOVE existing-target preservation to the object writer. |
| `locator_reservation_refusal_prevents_ancestor_storage_and_target_creation` | DELETE synthetic migration allocation/allowance protocol. |
| `original_refusal_and_relative_or_escaped_output_fail_closed` | MOVE path containment; DELETE the legacy original-context shim. |
| `bootstrap_uses_one_target_before_inventory_and_cannot_run_legacy_import` | DELETE source-text ordering/old-import guards; MOVE pre-effect budget and definite-versus-possible effect behavior to current owners. |

### `control_migration_inventory_owner_boundary.rs`

| Test | Disposition |
|---|---|
| `ordered_windows_pages_cover_more_than_one_page_exactly_once` | MOVE bounded stable enumeration to stopped-root collection if the current reader pages. |
| `same_handle_bytes_are_exact_and_over_limit_never_read_or_truncated` | MOVE exact bounded reads to the retained-object reader; the old test does not prove zero oversized I/O. |
| `missing_file_and_non_file_are_distinct_dispositions` | MOVE absent versus invalid-object distinction to the real reader. |
| `escaped_parent_is_refused_before_io` | MOVE containment to filesystem acquisition. |
| `original_context_refusal_stops_before_any_new_observation` | DELETE synthetic legacy-context protocol. |
| `page_callback_drift_cannot_turn_disappearance_into_complete_absence` | MOVE incomplete-enumeration behavior to stopped-root collection. |
| `visit_retains_leaf_directory_and_blocks_replacement_between_pages` | MOVE only if the selected walker spans observations; otherwise DELETE. |
| `multi_link_object_is_a_gap_and_no_content_is_returned` | MOVE alias-safe identity/uncertain-object preservation to acquisition/collection. |
| `stale_unmapped_preparation_is_not_a_current_link_and_dangling_links_are_refused` | DELETE old DIRECT link protocol; MOVE stale/dangling refusal to current preparation/object reading. |
| `unknown_control_representation_is_incomplete_instead_of_silently_skipped` | MOVE unknown-metadata blocking of collection; DELETE source-text assertions. |
| `empty_temporary_control_directory_has_a_metadata_row_before_recursion` | DELETE old recursive representation/source-shape assertion. |

No test above is retained unchanged. Receiving owners are the
[object store](../../crates/search-source/search-revision-store/FUNCTIONS.md),
[safe reader](../../crates/search-source/search-safe-reader/FUNCTIONS.md),
[materializer](../../crates/search-prep/search-materializer/FUNCTIONS.md) and
[stopped-root collector](../../crates/search-runtime/search-retention/FUNCTIONS.md).

## Remaining harness cuts

Still remove with their last real callers:

- `wave1`–`wave6` and optional old-package wiring;
- CLI/provider-client pairing/loopback compatibility;
- wave4/test provider router/capability machinery;
- old query/plan/handle/rebuild/proof production stacks;
- migration/quarantine/old-command fixtures;
- lexical fixture-backed activation/profile authority;
- unavailable model/document/adapter scaffolds when no selected consumer exists.

Do not replace the wave ladder with another acceptance feature, package map or all-tests gate.

## Target evidence portfolio

### Fast owner tests

Keep deterministic causal tests for typed redb compare/set and restart readback; immutable object
create/reuse/no-clobber/final verification; preparation empty/gap/coordinates; stable retrieval identity and
published evidence; real Qdrant conversion/recovery; view-guard cleanup; query-window commit after complete
output; framed protocol bounds/cancellation/partial I/O.

### One connected smoke

```text
initialize/open current root
→ retain one real file
→ restart and recover preparation
→ publish/read back native BM25 state
→ CLI Search
→ pinned evidence lookup
→ exact retained excerpt
```

A constructor, source-shape assertion, oracle or self-test string is not this smoke.

### Focused regressions after implementation

Edit/rename; explicit/empty/unknown scope; IDF population; old pinned view; unknown Qdrant write; lost-index
rebuild; cancellation/deadline; failed terminal output; bounded expansion; missing/corrupt evidence; exact
no-match; stopped-root collection; archive/import; selected document/parser cases.

R031 Rust 1.98 check and minimal Clippy passed for `search-materializer` and `search-unitizer` in
[run 40084893130](https://github.com/UnknownAlienHuman/eliot-search/actions/runs/40084893130). Tests were not run.
That evidence does not cover the daemon, current binaries, Windows, Qdrant or product behavior. R032 changes
only documentation and adds no compiler/runtime claim.
