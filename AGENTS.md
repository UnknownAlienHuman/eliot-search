# Working on ELIOT Search

Start with [docs/README.md](docs/README.md), then the
[implementation spine](docs/implementation/README.md) and only the nearest owner recipe needed for the current
vertical cut. Canonical docs own implementation direction; issues track responsibility/acceptance, not another
architecture or prerequisite graph.

## One manager, one vertical cut

Use one manager-owned worktree. The manager owns interfaces, integration, deletion of displaced routes and commit.
Parallel workers may inspect/change disjoint files but must not invent overlapping interfaces or second paths.

Before code, name:

1. real entrypoint;
2. concrete owner/effect;
3. immediate real consumer;
4. displaced route removed by this or the next bounded cut;
5. smallest locked Rust 1.98 check/Clippy closure.

Then implement. Do not restart a repository-wide audit, create a control plane or block on optional donors, broad
tests, unrelated issues or private #598 work.

## Current delivery order

```text
1 Foundation
→ 2 one file / ObjectRef / retrieval_text
    / vectorless Qdrant 1.20 scoring text index / TextQuery / CLI excerpt
→ 3A restart + authoritative inventory + complete fresh generations
    + predeclared release measurements
→ 3B source-local point incrementality only where 3A budgets fail
→ 4 Exact/live session
→ 5 maintenance and optional depth
```

Foundation ships only root/plain settings. Plain storage is baseline; #598 encryption is optional.

## Active Qdrant/query instructions

- Upgrade the current client 1.19 lock, server, schema and real consumers together to one exact 1.20.x patch.
- Use a vectorless collection with one `retrieval_text` payload field.
- Create scoring Text index before ingestion: Code tokenizer, lowercase, no stemmer/stopwords/ascii folding.
- Query using `TextQuery` with explicit profile `k`/`b`.
- Build with explicit `InsertOnly` and `wait`; readiness includes a deterministic TextQuery canary, not just point
  readback.
- Do not preserve sparse `qdrant/bm25` inference, fixed `avg_len`, local postings or automatic identifier aliases.
- Free text is any-term nomination. Required/excluded clauses are explicit filters; punctuation-sensitive search is
  Exact.
- Base selected source/access population affects both candidates and IDF; request-only narrowing affects candidates
  only. Whole-collection selection omits redundant filter/IDF.
- Stage 2/3A projection builds complete generations with deterministic generation-bound IDs. No old/new occurrence
  matching, sequence or point closure.
- Baseline cleanup is whole physical collection only; delete the old point-reclaimer platform after moving the
  small collection delete/absence helper.
- `PublishedView` pins exact physical collection + retrieval profile + redb snapshot.
- No aliases, custom-shard portfolio, Edge fallback, backend trait or dual writer.

## Platform publication

On Windows root/object finalization is tied to the opened stage handle and retained destination parent. Current
path-based `tempfile::persist_noclobber` is not trusted as final authority. Implement/run one owner-private native
canary. No generic filesystem abstraction or silent path fallback.

## Lifetime and lock discipline

- `DataRootGuard` surrounds process/store lifetime, not individual operations.
- One publisher selects work; no redb writer/view mutex spans object/Qdrant I/O.
- Query pins one view under a short lock, then releases it before scope/backend/evidence work.
- Redb writers perform bounded compare/commit only.
- Reserve request/output capacity before retrieval; mutate window/cursor only after complete output.
- Last guard drop wakes cleanup and never deletes inline.

Follow the exact partial order in the implementation spine. Remove/adapt an old API requiring reverse order.

## Verification and records

After production changes run the smallest locked Rust 1.98 check/Clippy for changed owners and immediate consumers.
Focused runtime cases follow when the path exists; broad release/scale evidence does not precede the first result.

For every published slice:

1. verify actual PR head and intervening commits;
2. record source SHA, inspected paths, finding, decision, commands/results and unrun behavior in
   [the registry](docs/audit/ANALYSIS_REGISTRY.md);
3. update canonical owner docs and conflicting issues in the same slice;
4. synchronize #352 and PR #610 when stage changes;
5. report only checks which ran.

Resume from the [R051 registry](docs/audit/ANALYSIS_REGISTRY.md#resume), not chat summaries or historical issues.
