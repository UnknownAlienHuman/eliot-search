# Source reconciliation — observations, not receipts

One daemon scheduler turns selected-root observations into current source/head changes. The
[implementation guide](../../../docs/implementation/README.md) owns the application path;
[source identity](../search-source-identity/FUNCTIONS.md) owns logical-source matching.
Reviewed at `c0fcf2947b0ce6118ae67ee94e557632bd2a809d` on 2026-10-23. Target rules below are not implemented behavior.

## Source facts and defects to avoid copying

| Inspected code | Finding |
|---|---|
| `src/lib.rs::reconcile_source` | Pure comparison, not a walker or durable commit. `Missing` produces `RetireMissing` before the root-binding check in `reconcile_present`. A matching declared source/receipt does not repair this missing check. |
| `reconcile_present`, `(None, Some(file), false, false \| true)` | First physical-key observation returns only `BindStableFileIdentity`, even when digest/size changed. The returned action omits the concurrent content update. |
| `reconcile_present`, same path with changed physical key | Unconditionally retires/nominates a replacement, contrary to the current logical-source rule. An editor's atomic save can replace the OS file without replacing the logical source. |
| `reconcile_present`, changed content without a physical key | Refuses the update. Lack of rename evidence must not prevent a verified same-locator content update under the selected logical-source policy. |
| `denial_as_error` / `quarantine_as_error` | Discard the supplied cause and return `ReasonSetInvalid`, including ordinary oversized input, missing rename evidence and root mismatch. |
| `ReconcileLedger::record` | Retains operation IDs/digests/outcomes in a vector, with a default lifetime ceiling of 2,000,000. Not a work inventory or durable recovery mechanism. |
| `search-source-records/src/root_currentness.rs` | A normal hint increments `watcher_sequence` but does not invalidate sync. `current_workspace_truth` ignores pending hints; `mark_reconciled_synced` checks neither hints nor a captured scan. |
| Same currentness owner | `reconcile` clears hints/overflow from root availability alone. Hint positions are not rebased when a root is inserted/removed. Do not reuse vector slots as work identities. |
| daemon `source_roots/kernel/catalog.rs::refresh` | Probes configured directories, then calls that `reconcile`; it does not inventory their files. Several catalog mutation/view methods are test-only; watcher methods have no connected native producer in the inspected composition. |
| daemon `owner_composition/kernel/source_sync.rs::run_source_sync_preflight` | Even successful preflight returns `SOURCE_ROOT_SYNC_EFFECTS_UNAVAILABLE`. The defects above are source-level counterexamples, not a reproduced live loss of indexed files. |

Example: reconcile one available root, mark it synced, then call `note_watcher_hint(0, Modified)`.
The current pure owner still returns `workspace_current=true`. A passing test of its original happy path
cannot establish freshness. Old W5/P09 docs required scope-bound observations and incomplete-scan handling,
but their receipts, port catalogue and immutable-observation platform did not implement those requirements.

## One guarded source update

Capture the actual root registration, selection policy and expected current source/binding when starting
work. Validate source AND root association before every presence branch, including absence. Apply only while
those captured values still match; otherwise reread/requeue. Root removal/re-registration at the same path
must not accept old work. Use existing current rows and a short compare/set, not a new operation archive.

Evaluate path, physical-key and content changes independently. Commit their applicable changes together;
learning a physical key must not suppress a changed head. Preserve the identity owner's same-locator rule
for atomic saves. Physical keys assist unambiguous moves; missing keys are not a reason to freeze ordinary
content updates, and equal bytes alone never prove a rename. No source identity history is needed.

Return the real failure/retry reason. A limit, stale observation, unavailable path or identity ambiguity is
not malformed configuration and not automatic quarantine. Keep the existing retained revision readable;
record the current gap instead of manufacturing empty content or a successful update.

## Omission is not absence

An ignore-filtered walk describes a selection, not all physical files. Handle these outcomes distinctly:

| Observation | Source-owner action |
|---|---|
| Admitted, stable bytes read | Retain those exact bytes and compare/set the current head. |
| Explicitly excluded by the current valid selection policy | Update desired membership/reason; ordinary publication removes it from new selected views. Do not claim physical deletion or delete retained bytes. |
| Final check confirms absence inside an available, unchanged registered root | Retire only the expected current source row. A competing newer row wins. |
| Permission/I/O failure, disconnected root, uncertain policy, cancellation or work ceiling | Preserve an explicit affected-scope gap. No deletion from omission and no complete-currentness claim. |

For previously registered rows not seen by discovery, use bounded current-root table iteration and a final
root-relative policy/path check before retirement. Do not turn `old rows - yielded paths` directly into a
delete batch. Hidden/type/size filters, changed ignore rules and nested-root exclusions can all omit a live
file. A processing budget is incomplete work, not a source-selection decision. A missing root is not proof
that each of its children was deleted; explicit unregister is a separate operation.

A filesystem walk is not an atomic filesystem snapshot. It may apply individually verified changes while
other files change. Late work cannot overwrite a newer row or close a newer gap. Exact absence claims refer
to the actually evaluated retained inventory, not to all live filesystem bytes at response time.

## Discovery and direct hints use the same selection

Use the reviewed `ignore::WalkBuilder`; do not invent an ignore grammar. Make hidden files, parent/global
rules, `.ignore`, `.gitignore`, nested roots, links and own-storage exclusions explicit.

The donor has separate error channels: builder/add-ignore failure, traversal `Err`, and `DirEntry::error()`
on an otherwise successful entry. Ignore parsing can partially succeed. Inspect all channels. Do not silently
accept a partial rule set as complete policy, or admit new bodies whose selection is unresolved. Keep the
affected subtree incomplete; unaffected roots may continue. Iterator exhaustion alone is not success.

In the reviewed serial walker, `skip_entry` immediately accepts depth-zero paths. Therefore a dirty file
passed as a new walk root is not equivalent to discovering it under its registered root. Evaluate direct
hints with the same root-relative ancestor/ignore context; an event is not an explicit user override. When
that context cannot be reproduced cheaply, schedule the affected configured-root scan rather than add a
second matcher. Metadata `max_filesize` is also only a discovery filter; the reader enforces actual bytes.

An ignore/config change schedules reevaluation of both existing selected rows and newly discoverable paths.
If parent/global ignore sources are enabled, periodic/explicit sync reloads them too; watching only the root
cannot observe every external rule change. Configuration changes during work invalidate only the affected
work, not every root in the application. No rule digest or policy receipt is required.

## One finite scheduler and honest freshness

Startup, explicit sync and periodic reconciliation work without a watcher. Optional #239 adds responsiveness,
not correctness authority. Use stable current root IDs in work items, never sorted catalog positions.

Coalesce duplicate paths and atomic-save bursts. Overflow/error/uncertain hints set an affected-root dirty
latch. A callback does no read/database work. A small safe latch lifecycle is:

```text
start scan: take pending work / clear the prior latch; mark this root active
while active: new hints set pending/dirty again
finish: preserve new pending/dirty; failure or stale registration/policy sets dirty again
```

Do not clear a newly raised latch at completion, or clear all hints merely because the root directory exists.
A drained queue is not completion while a worker is active. A sync result covers its captured work only;
known pending/active/failed work prevents claiming the root is caught up. Silent lost notifications still
require periodic/startup repair. Reuse process-local scheduling state; no durable hint/cursor/lease database.

Stream entries/errors into bounded downstream work and use one shared thread budget. `build_parallel` order
is not deterministic; sorting an entire portfolio is not required. `walkdir::max_open` bounds descriptors,
not memory. Do not add `PollWatcher` on top of the same periodic inventory by default, or adopt debouncer
`FileIdMap`'s whole-tree link-following cache. Raw RecommendedWatcher is enough when notifications are selected.

Compare actual transform inputs before reuse, not just source content hashes. A path-only rename can reuse
parsing only if parser/profile/context inputs are unchanged; extension-selected formats or path-derived
retrieval prefixes can change semantics with identical bytes. Publication handles the resulting delta;
reconciliation does not rewrite live Qdrant points directly. The durable preparation/outcome comparison in
the materializer recipe repairs lost queue wake-ups without another job ledger.

## Git views

Working-tree bytes, staged index blobs and committed tree blobs are distinct views. Use the controlled
[safe-reader plumbing](../search-safe-reader/FUNCTIONS.md): exact tree/index inputs, initial `ls-tree`, raw
`diff-tree --no-renames` and bounded batch reads of changed blobs. Unresolved stages and missing local objects
are gaps; no checkout/hooks/filters, lazy fetch or implicit submodule traversal. A later HEAD cannot relabel
old reads. Do not make a second Git backend merely to satisfy an interface.

## Donor evidence and remaining execution

Reviewed source, not a dependency upgrade:

- [ignore walk at 4649aa9](https://github.com/BurntSushi/ripgrep/blob/4649aa9700619f94cf9c66876e9549d83420e16c/crates/ignore/src/walk.rs): `DirEntry::error`, `add_ignore`, `Walk::skip_entry`, filtering and traversal.
- [Published ignore 0.4.33 DirEntry docs](https://docs.rs/ignore/0.4.33/ignore/struct.DirEntry.html#method.error) and [WalkBuilder docs](https://docs.rs/ignore/0.4.33/ignore/struct.WalkBuilder.html#method.add_ignore): separate entry errors and partial parsing, cross-checked 2026-10-23.
- [walkdir](https://github.com/BurntSushi/walkdir/blob/4f26be4d450910916ea11533b2efc52b9a6483bc/src/lib.rs), [notify Windows](https://github.com/notify-rs/notify/blob/a1d7c2d8f80786679d58ec6d5986a1d4278bc8cf/notify/src/windows.rs), [poll](https://github.com/notify-rs/notify/blob/a1d7c2d8f80786679d58ec6d5986a1d4278bc8cf/notify/src/poll.rs), [debouncer FileIdMap](https://github.com/notify-rs/notify/blob/76f48c70bbf7fed75440d580e498e90cbc6ce140/notify-debouncer-full/src/file_id_map.rs): earlier inspected costs; not newly re-audited in this pass.
- [gix low-level tree comparison](https://github.com/GitoxideLabs/gitoxide/blob/ad0202e3ab396d752a56664001c134f48d834177/gix-diff/src/tree/function.rs): future alternative only if packaging requires no installed Git; equal-OID subtrees can be skipped. Not a second selected backend.

After the connected implementation, exercise wrong-root absence; first physical key plus changed bytes;
atomic save without source churn; no-key same-path update; actual error causes; hint-after-sync and hint-during-
scan; registration reordering; invalid ignore on an Ok entry; direct-hint ignore parity; formerly selected
file becoming ignored/oversize; inaccessible root; partial scan; and rename changing transform context.
Run minimal pinned compiler/Clippy after code. None of these runtime outcomes is claimed by this static pass.
