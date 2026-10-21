# Audit coverage and remaining documentation work

Source baseline: `c8e38b6`. Replacement documentation re-reviewed on 2026-10-21, most recently from
`7b15dd6` through `8e603e0`. This is a broad static review with explicit remaining checks, not an exhaustive
line-by-line audit, runtime qualification or permission gate. The earlier blanket closure claim remains withdrawn.

## Where implementation starts

[Architecture](../architecture/README.md) states purpose; [implementation](../implementation/README.md) selects
mechanisms. A local FUNCTIONS map owns its operation recipe. READMEs/AGENTS/issues route to it rather than
copying another algorithm. This audit and the [workspace disposition](WORKSPACE_MEMBER_DISPOSITION_2026-10-21.md)
record evidence; package counts and audit statuses do not authorize or block code.

## Review coverage

| Area | Evidence available | Boundary |
|---|---|---|
| Workspace/entrypoints | Members, features, principal callers and competing applications inventoried | Not every Rust line or deletion consumer checked |
| Source/root/config/control | Source maps trace dropped body, replay/archive layers, root/settings blockers | Native/private object-key behavior unexecuted |
| Preparation/identity/Qdrant | Major implementations and selected upstream mechanisms inspected | No corpus or live-Qdrant measurement |
| Publication/views/GC | Effect ordering, old-view lifetime and real root dependencies reasoned through | Replacement code and fault behavior not executed |
| Query/session/protocol | Serving paths and replacement scenarios reviewed | No current end-to-end application run |
| Tests/tooling/optional features | Families and principal consumers inventoried | Not an exhaustive fixture or donor survey |
| Private #598 | Public contract only | Unpublished branch/worktrees untouched |

## Corrections to the replacement documentation itself

The prior `7b15dd6` pass removed a false audit-complete claim, map-as-authority hierarchy and repository-wide
#598 gate; corrected workspace counts; separated one-shot/session lifetime; and fixed Cancel observability,
blocked-I/O claims, capped serialization, window deadlines, handled-prefix cursor accounting and the false
equation of local flush with peer acknowledgement. Those changes remain in their operation owners.

The `8e603e0` pass addresses three remaining algorithmic holes and their immediate consumers:

| Fault | Consequence | Selected correction and owner |
|---|---|---|
| 'Sorted prepared stream' without an ordering producer; whole-view pages without a point index | Full-corpus sort/cache or an invented manifest storage engine | Source-local bounded sort/diff; compact redb source/evidence rows with exact-key/range lookup. [Planner](../../crates/search-index-qdrant/search-projection-planner/FUNCTIONS.md), [control](../../crates/search-control-redb/FUNCTIONS.md) |
| 'Stable anchor + duplicate position' treated as continuity | Inserted equal blocks shift positions or reuse the wrong occurrence | One-old/one-new input match only; ambiguous repeats get fixed new births. [Unitizer](../../crates/search-prep/search-unitizer/FUNCTIONS.md) |
| Full live-root traversal repeated under a lock before every deletion page | Work can scale with references times deletion pages; page size does not bound the writer pause | Explicit stopped-root maintenance, complete mark once, then exact enumeration/deletion. [Local GC](../../crates/search-runtime/search-retention/FUNCTIONS.md) |

Related corrections: ordinary metadata deltas commit atomically with current view; full rebuilds populate
separate generation tables; a view pins the actual read snapshot rather than using latest rows; partial
expected RPC/delete progress is retried per item rather than automatically classified as corruption.
[Publication](../../crates/search-index-qdrant/search-publication/FUNCTIONS.md) owns those rules.

Issues #259, #261, #300, #302, #303 and #329 were updated to reference these recipes rather than retain their
old duplicate algorithms. Other issue/local-document bodies are not claimed completely re-swept in this pass.

## Source and donor evidence used in this pass

The planner's current `manifest_read.rs` materializes/re-encodes a complete manifest; the updated source maps
identify its callers and lost vector inputs. `search-control-redb/Cargo.toml` already pins redb 2.6.3.
Checked redb tag v2.6.3 at `f4e3eb69dc2a4b4e01fefff0b9e039c6c7ab98f6`: read transactions capture the table-tree root
and table handles retain the transaction guard. Official 2.6.3 docs provide get/range and describe copy-on-write
MVCC. Exact links and the selected table boundary are in the control map. No dependency upgrade was made.

## Costs and limits not to hide

- Conservative matching may rewrite repeated units in an edited source; it does not promise universal ID reuse.
- Source-local work is bounded by a supported source size/unit count, not constant memory for an unlimited file.
- Incremental metadata commits have a prechecked ceiling; oversized single-source updates remain explicit gaps.
- Old redb readers retain old pages. Disk/backlog behavior must be measured with stalled publication and windows.
- Local GC requires downtime and temporary mark-table disk. It is not online or zero-cost; disk pressure pauses
  writes rather than authorizing eviction. Qdrant cleanup still operates with live-view eligibility.
- Platform I/O cancellation, Windows namespace durability, Qdrant behavior, recovery and quality/scale are unrun.

## Remaining checks

Check residual same-level README/AGENTS/issue duplication against the selected owner maps, especially old
manifest-page, source-GC and snapshot wording. Do not solve contradictions by implementing both versions.
Confirm the useful private storage handoff against the simplified boundary without importing its retired
six-domain/purge/restore obligations. Preserve navigation/comparison/document/large-corpus goals while
removing their broken scaffolds. Actual compilation, runtime faults and measurements follow implementation;
this record must not manufacture evidence that does not exist.

Historical Architecture 8.4, waves, swarm, qualification and package-status material remains evidence only.
Git history preserves old recipes; no additional approval database or audit wave is needed to start an
independent concrete change.
