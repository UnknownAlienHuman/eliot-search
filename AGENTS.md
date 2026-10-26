# Working on ELIOT Search

Start with [docs/README.md](docs/README.md), then the
[implementation spine](docs/implementation/README.md) and only the nearest owner recipe required for the current
vertical cut. Canonical docs own direction; issues track responsibility/acceptance, not another architecture or
prerequisite graph.

## One manager, one vertical cut

Use one manager-owned worktree. The manager owns interfaces, integration, removal of displaced routes and commit.
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
    / explicit vectorless Qdrant 1.20 Text index / TextQuery canary/query / CLI excerpt
→ 3A restart + inventory + explicit filter/residency profile
    + complete fresh generations + predeclared release budgets
→ 3B source-local point incrementality only where 3A budgets fail
→ 4 Exact/live session
→ 5 maintenance and optional depth
```

## Active Qdrant/query rules

- Upgrade current client 1.19 lock, server, schema and real consumers together to one exact 1.20.x patch.
- Stage-2 physical Text index is fully explicit: Code tokenizer, lowercase, no ASCII folding/phrase/stemmer/
  stopwords/token limits/nested path, BM25 scoring, `enable_hnsw=false`, `memory=Pinned`.
- Hidden Qdrant defaults are not a profile. Stage 3A explicitly selects `Pinned|Cached|Cold` and measures it.
- Stage 2 one-file collection creates no filter indexes. Stage 3A creates only fields consumed by real scope clauses;
  keyword/filter indexes disable HNSW and set memory explicitly.
- Query scoring is separate: k=1.2, b=0.75, finite/range validated, no score threshold. Changing only k/b does not
  rebuild the collection.
- Build with InsertOnly + wait; readiness requires exact schema/point/count plus deterministic TextQuery canary.
- Code tokenizer already supplies subwords and joined camel/snake tokens to ranking. No local identifier aliases.
- Free text is any-token nomination. Full-text required/excluded filters, if exposed, are all-subword predicates,
  not exact identifiers or phrases. Exact punctuation/location uses retained-source Exact.
- Whole-collection selection omits filter/IDF. Later source/access base population appears in candidates and IDF;
  request-only narrowing is candidate-only.
- Query requests no vectors and only evidence-routing payload; excerpts come from immutable objects.
- Stage 2/3A uses complete generations and generation-bound IDs, no occurrence matching/sequence/closure.
- Baseline cleanup is whole collection. No aliases, custom-shard portfolio, Edge fallback or second backend.

## Platform publication

Windows root/object finalization is tied to opened stage handle and retained parent. Current path-based
`tempfile::persist_noclobber` is not trusted final authority. Implement/run one owner-private native canary. No
generic filesystem abstraction or silent fallback.

## Lifetime/lock discipline

- `DataRootGuard` surrounds process/store lifetime.
- Publisher holds no redb writer/view mutex during object/Qdrant I/O.
- Query pins view under a short lock and releases it before scope/backend/evidence work.
- Redb writers perform bounded compare/commit only.
- Reserve request/output capacity before retrieval; mutate window/cursor only after complete output.
- Last guard drop wakes cleanup and never deletes inline.

Follow the partial order in the implementation spine; remove/adapt an old API requiring reverse order.

## Verification and records

After production changes run smallest locked Rust 1.98 check/Clippy for changed owners and immediate consumers.
Focused runtime cases follow when the path exists; broad release/scale evidence does not precede the first result.

For every published slice:

1. verify actual PR head/intervening commits;
2. record source SHA, inspected paths, finding, decision, commands/results and unrun behavior in
   [the registry](docs/audit/ANALYSIS_REGISTRY.md);
3. update owner docs and conflicting issues in the same slice;
4. synchronize #352 and PR #610 when stage changes;
5. report only checks which ran.

Resume from the [R052 registry](docs/audit/ANALYSIS_REGISTRY.md#resume), not chat summaries or historical issues.
