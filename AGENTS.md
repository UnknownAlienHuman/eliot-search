# Working on ELIOT Search

Start with [docs/README.md](docs/README.md), then read
[architecture](docs/architecture/README.md), the
[implementation spine](docs/implementation/README.md), and only the nearest owner recipe needed for the current
vertical cut.

Canonical documentation owns implementation direction. GitHub issues track responsibility/acceptance; they are
not an independent architecture or prerequisite graph. If an issue conflicts with architecture, implementation
spine or nearest owner recipe, correct it before coding. Historical waves, packets, receipts and immutable audit
links are evidence only.

## One manager, one vertical cut

Use one manager-owned worktree. The manager owns integration, interface decisions and commit. Parallel workers
may inspect or change disjoint files but must not invent overlapping interfaces, merge their own branches or
create a second product path.

Before code, name:

1. real entrypoint;
2. concrete owner/effect;
3. immediate real consumer;
4. obsolete route removed by this cut or the next explicitly bounded cut;
5. smallest locked Rust 1.98 `check`/Clippy closure.

Then implement. Do not restart a repository-wide audit, create another control plane or block code on package
counts, optional donors, broad tests, unrelated issues or private work.

## Current delivery order

```text
1 Foundation
→ 2 one explicit file / plain ObjectRef / preparation / fresh generation / Search / CLI excerpt
→ 3A restart + inventory + complete fresh-generation convergence
→ 3B optional measured point incrementality
→ 4 Exact/live session
→ 5 maintenance and selected depth
```

Foundation ships only invocation, root and plain-storage settings. Source/Qdrant/query/optional fields arrive
with their real consumer. Empty-root initialization publishes the final manifest last and uses exact
discard/retry, not a resume/replay state machine.

Plain immutable storage is baseline. #598 is an optional encrypted profile after an accepted public handoff; it
cannot block source retention, preparation, publication, query or CLI. Do not inspect or modify its private
branches/worktrees.

Issues are ownership trackers, not prerequisite order. The walking skeleton and fresh-generation convergence use
final owners and persisted values; neither is a throwaway.

## Fresh generation before incremental mutation

Walking skeleton and stage 3A build a complete never-reused physical Qdrant collection, settle/read back the
build, commit matching evidence/current state, then explicitly install a `PublishedView` naming that physical
collection. Visible fresh-generation collections are immutable. A source change builds the next generation.

Do not begin stable cross-revision point identity, `valid_until` closures or compaction heuristics before stage 3A
works and full-rebuild cost is measured. If complete rebuild is sufficient, do not ship stage 3B complexity.

If stage 3B is selected:

- new points explicitly use `insert_only`; default upsert is forbidden;
- closure is one fixed non-creating exclusive `valid_until` update;
- individual points are never physically deleted;
- profile/schema change or measured closed-point growth builds a fresh generation.

`PublishedView` names the exact physical collection. A Qdrant alias is not serving/recovery authority.

## Lifetime and lock discipline

- `DataRootGuard` is outer process lifetime, not a serializable lease.
- Publication has one logical task/permit but holds no redb writer or view-manager mutex across object/Qdrant I/O.
- Pin/install `PublishedView` under a short manager lock and release before I/O.
- Redb writers contain only bounded metadata compare/commit.
- Query reserves capacity, pins one view, performs backend/evidence work, then mutates window/cursor only for a
  bounded final commit after complete output.
- Last guard drop wakes cleanup; it never deletes inline.
- Never add a reverse lock path to satisfy an old fixture.

## Object/root filesystem boundary

Prefer same-directory unique staging, streaming identity, file sync, no-clobber finalization and exact final
readback. `tempfile::persist_noclobber` is the selected candidate, not yet installed. If Windows handle/no-reparse
semantics require a native primitive, keep one tiny finalizer inside the owning root/object module; do not create a
generic filesystem framework.

Advertise only executed durability. Namespace readback and logical Qdrant absence are not physical-erasure or
sudden-power-loss proof.

## Backend and watcher boundary

Qdrant is the only selected indexed backend. Tantivy/Zoekt are design donors, not fallback implementations; add no
backend trait or dual writer before measurement. Watcher events only dirty a stable root ID. Startup/explicit/
periodic authoritative inventory establishes selected state and handles overflow/uncertainty.

## Verification and records

After production changes run the smallest locked Rust 1.98 compiler/Clippy closure for changed owners and
immediate consumers. Focused runtime cases follow when the path exists; broad suites, native crash matrices,
relevance and scale do not block the first connected path.

For every published slice:

1. verify actual PR head and intervening commits;
2. record source SHA, inspected paths, finding, decision, changed owner, commands/results and unrun behavior in
   [the registry](docs/audit/ANALYSIS_REGISTRY.md);
3. update canonical owner docs and conflicting open issues in the same slice;
4. synchronize #352 and PR #610 when current stage changes;
5. report only checks which actually ran.

Work remains in [PR #610](https://github.com/UnknownAlienHuman/eliot-search/pull/610). Resume from the
[registry](docs/audit/ANALYSIS_REGISTRY.md#resume), not chat summaries, historical issue bodies or default-branch
code search.