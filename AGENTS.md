# Working on ELIOT Search

Start with [docs/README.md](docs/README.md), then read the
[architecture](docs/architecture/README.md), the
[implementation spine](docs/implementation/README.md), and only the nearest owner recipe needed for the current
vertical cut.

Canonical documentation owns current implementation direction. GitHub issues track responsibility and acceptance;
they are not an independent architecture or prerequisite graph. If an open issue conflicts with the architecture,
implementation spine or nearest owner recipe, correct that issue before coding. Historical packets, waves,
receipts and immutable audit links are evidence only.

## One manager, one vertical cut

Use one manager-owned worktree. The manager owns the integration boundary, interface decisions and commit.
Parallel workers may inspect or change disjoint files, but must not invent overlapping interfaces, merge their own
branches or create a second implementation path.

Before code, name:

1. the real entrypoint;
2. the concrete owner/effect being changed;
3. the immediate real consumer;
4. the obsolete path removed by the same cut or the next explicitly bounded cut;
5. the smallest locked Rust 1.98 `check` and Clippy closure.

Then implement the path. Do not restart a repository-wide audit, create another control plane, or block code on
package counts, optional donors, broad tests, unrelated issues or private work.

## Current delivery order

```text
Foundation
→ one explicit file / plain ObjectRef
→ one preparation result
→ one fresh Qdrant generation
→ one one-shot Search
→ pinned retained excerpt
→ CLI output
→ convergence/restart
→ Exact/live session
→ maintenance and optional depth
```

Foundation ships only invocation, root and plain-storage settings. Settings for source, Qdrant, query and optional
profiles are added with their real consumer.

Plain immutable storage is the baseline. #598 owns an optional encrypted profile after an accepted public
handoff; it is not a prerequisite for source retention, preparation, publication, query or CLI. Do not inspect or
modify #598 private branches or worktrees.

Issues are ownership trackers, not prerequisite order. One vertical implementation cut may touch several issues.
The one-file walking skeleton uses final owners and data values; it is not a throwaway prototype.

## Lifetime and lock discipline

- `DataRootGuard` is the outer process lifetime, not a serializable lease.
- Publication has one logical owner/task. Do not hold a redb writer, view-manager mutex or query-window borrow
  across Qdrant/object/process I/O.
- Pin/install a `PublishedView` under a short manager lock and release it before I/O.
- Redb writers contain only bounded metadata comparisons/commits; all object and RPC work occurs outside them.
- Query reserves finite capacity, pins one view, performs backend/evidence work, then acquires a window/output
  mutation only for the bounded final commit.
- Lock acquisition must follow the order documented in the implementation spine; never add a reverse path to
  satisfy an old fixture.

## Qdrant baseline

New points are immutable and explicitly use `insert_only`; default upsert is forbidden for publication retry.
A closure is one non-creating fixed `valid_until` update. Individual point IDs are never physically deleted.
Rebuild/compaction switches to a never-reused generation. Whole-generation cleanup occurs only after no live
view/window uses it, no new publication can target it, collection deletion returns terminally and exact
collection-absence observation succeeds. Until then retain the cleanup row and apply backpressure. Never reuse a
collection name or claim physical disk erasure from logical absence.

## Verification and records

After production changes, run the smallest locked Rust 1.98 compiler/Clippy closure for changed owners and
immediate consumers. Focused runtime cases follow when that path exists; broad suites, Windows crash matrices,
relevance and scale do not block writing the first connected path.

For every published slice:

1. verify the actual PR head and intervening commits;
2. record source SHA, inspected paths, finding, decision, changed owner, commands/results and unrun behavior in
   [the registry](docs/audit/ANALYSIS_REGISTRY.md);
3. update the canonical owner docs and any conflicting open issue in the same slice;
4. synchronize #352 and PR #610 when current stage changes;
5. report only checks which actually ran.

Preparation and implementation remain in [PR #610](https://github.com/UnknownAlienHuman/eliot-search/pull/610).
Resume from the [registry](docs/audit/ANALYSIS_REGISTRY.md#resume), not from chat summaries, historical issue
bodies or default-branch code search.
