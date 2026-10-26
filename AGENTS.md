# Working on ELIOT Search

Start with [docs/README.md](docs/README.md), then the
[architecture](docs/architecture/README.md), the
[implementation spine](docs/implementation/README.md), and only the nearest owner recipe needed for the current
vertical cut.

Canonical documentation owns current direction. Issues track responsibility and acceptance; they are not an
independent architecture or prerequisite graph. Historical packets, waves, receipts and immutable audit links are
evidence only.

## One manager, one vertical cut

Use one manager-owned worktree. The manager owns interfaces, integration and commit. Parallel workers may inspect
or change disjoint files, but must not invent overlapping interfaces, merge their own branches or create a second
implementation route.

Before code, name:

1. real entrypoint;
2. concrete owner/effect;
3. immediate real consumer;
4. displaced route removed by this cut or next bounded cut;
5. smallest locked Rust 1.98 `check` and Clippy closure.

Then implement. Do not restart a repository-wide audit, create another control plane or block code on package
counts, optional donors, broad tests, unrelated issues or private work.

## Current delivery order

```text
1 Foundation
→ 2 one file / ObjectRef / preparation
    / Qdrant 1.20 scoring full-text generation / TextQuery / CLI excerpt
→ 3A restart + authoritative inventory + complete fresh-generation convergence
    + predeclared release measurements
→ 3B source-local point incrementality only where 3A budgets fail
→ 4 Exact/live session
→ 5 maintenance and optional depth
```

Foundation ships only root/plain settings. Plain storage is baseline; #598 encryption is optional.

## Active Qdrant instructions

- Upgrade current locked client 1.19.0, server, schema and real consumers together to 1.20.x.
- Store retrieval text once in payload; create a scoring full-text index with explicit `code` tokenizer/text
  settings; query with `TextQuery`.
- Do not preserve baseline sparse BM25 inference, local fixed `avg_len`, aliases, custom-shard portfolio or Edge
  fallback.
- Stage 2/3A projection builds complete generations. Point IDs are deterministic and generation-bound.
- Do not start the old changed-source diff, stable occurrence identity, retain/create/close or sequence-range code
  before Stage 3A measurements activate Stage 3B.
- Baseline cleanup is whole physical collection only. Move the small delete/absence helper from the old reclaimer
  and delete the platform; never recreate point cleanup/writer ledgers.
- `PublishedView` pins exact physical collection + redb snapshot. Stage 3A has no sequence.

## Platform publication

On Windows, root/object finalization is tied to the opened stage handle and retained destination parent. Current
`tempfile::persist_noclobber` uses path-based `MoveFileExW`; do not trust it as final authority. Implement and run
one owner-private handle-based canary. No generic filesystem abstraction or silent path-only fallback.

## Lifetime and lock discipline

- `DataRootGuard` is the process lifetime, not a lease.
- One publisher selects work; no redb writer or view-manager mutex spans object/Qdrant I/O.
- Query pins one view under a short lock and releases it before backend/evidence work.
- Redb writers contain bounded compare/commit only.
- Query reserves finite capacity before retrieval and mutates a window/cursor only after complete bounded output.
- Last guard drop wakes cleanup; it never deletes inline.

Follow the exact order in the implementation spine. Remove/adapt an old API requiring a reverse order.

## Verification and records

After production changes, run the smallest locked Rust 1.98 compiler/Clippy closure for changed owners and
immediate consumers. Focused runtime cases follow when the path exists; broad crash/relevance/scale suites do not
block the first connected result.

For every published slice:

1. verify actual PR head and intervening commits;
2. record source SHA, inspected paths, finding, decision, commands/results and unrun behavior in
   [the registry](docs/audit/ANALYSIS_REGISTRY.md);
3. update canonical owners and conflicting open issues in the same slice;
4. synchronize #352 and PR #610 when current stage changes;
5. report only checks which actually ran.

Resume from the [R050 registry](docs/audit/ANALYSIS_REGISTRY.md#resume), not chat summaries or historical issues.
