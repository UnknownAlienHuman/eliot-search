# Working on ELIOT Search

Start with [docs/README.md](docs/README.md), then the
[implementation spine](docs/implementation/README.md) and only the nearest owner recipe required for the current
vertical cut. Canonical docs own direction; issues track responsibility and acceptance, not another architecture
or prerequisite graph.

## One manager, one vertical cut

Use one manager-owned worktree. The manager owns interfaces, integration, removal of displaced routes and the
commit. Parallel workers may inspect or change disjoint files, but must not invent overlapping interfaces or a
second product path.

Before code, name:

1. the real entrypoint;
2. the concrete owner/effect;
3. the immediate real consumer;
4. the displaced route removed by this or the next bounded cut;
5. the smallest locked Rust 1.98 `check` and Clippy closure.

Then implement. Do not restart a repository-wide audit, create another control plane or block on optional donors,
broad tests, unrelated issues or private #598 work.

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

- Upgrade the current client 1.19 lock, server, schema and real consumers together to one exact 1.20.x patch.
- Stage-2 Text index is fully explicit: Code tokenizer, lowercase, no ASCII folding/phrase/stemmer/stopwords/token
  limits/nested path, BM25 scoring, `enable_hnsw=false`, `memory=Pinned`.
- Stage 2 creates no filter indexes. Stage 3A adds only fields consumed by real scope clauses and sets HNSW/memory
  explicitly.
- Query scoring is separate: k=1.2, b=0.75, finite/range validated, no score threshold. Changing only k/b does not
  rebuild the collection.
- Build with InsertOnly + wait; readiness requires exact schema/point/count plus deterministic TextQuery canary.
- Code tokenizer supplies subwords and joined camel/snake tokens to ranking. Do not add local identifier aliases.
- Free text is any-token nomination. Exact punctuation/location uses retained-source Exact.
- Whole-collection selection omits filter/IDF. Later source/access base population affects candidates and IDF;
  request-only narrowing is candidate-only.
- Query requests no vectors and only evidence-routing payload; excerpts come from immutable objects.
- Stage 2/3A uses complete generations and generation-bound IDs, with no occurrence matching/sequence/closure.
- Baseline cleanup is whole collection. No aliases, custom-shard portfolio, Edge fallback or second backend.

## Windows publication — corrected R054 boundary

R053 correctly rejected the reachable legacy relative-root Win32 wrapper, but incorrectly selected a private
`NtSetInformationFile`-style user-mode operation as the baseline. Do not use that private NT call.

Use one documented owner-private Win32 operation:

```text
retain and verify every directory handle from data root to final parent
  (share read/write, do not share delete)
+ retain the opened stage handle with DELETE access
+ derive one canonical absolute final path from the retained final-parent handle
→ SetFileInformationByHandle(FileRenameInfo)
     RootDirectory = NULL
     ReplaceIfExists = FALSE
→ reopen final relative to the retained parent
→ verify file identity, kind, length and bytes
```

The omitted `FILE_SHARE_DELETE` locks prevent rename/delete/replacement of every retained path component while the
operation runs. The final path format, access/share flags and supported local filesystem are selected only after a
focused canary. Existing destination must fail; failure leaves the stage classifiable. If the documented operation
cannot be demonstrated for the advertised profile, refuse that profile.

Do not fall back to path-based `MoveFileExW`, relative-`RootDirectory` Win32 wrapper, private NT file-information
calls or a generic filesystem framework. Remove the legacy source-registration publication route with callers.

## Lifetime/lock discipline

- `DataRootGuard` surrounds process/store lifetime.
- Publisher holds no redb writer or view mutex during object/Qdrant I/O.
- Query pins a view under a short lock and releases it before scope/backend/evidence work.
- Redb writers perform bounded compare/commit only.
- Reserve request/output capacity before retrieval; mutate window/cursor only after complete output.
- Last guard drop wakes cleanup and never deletes inline.

Follow the partial order in the implementation spine; remove or adapt an old API requiring reverse order.

## Verification and records

After production changes run the smallest locked Rust 1.98 check/Clippy for changed owners and immediate
consumers. Focused runtime cases follow when the path exists; broad release/scale evidence does not precede the
first result.

For every published slice:

1. verify the actual PR head and intervening commits;
2. record source SHA, inspected paths, finding, decision, commands/results and unrun behavior in
   [the registry](docs/audit/ANALYSIS_REGISTRY.md);
3. update owner docs and conflicting issues in the same slice;
4. synchronize #352 and PR #610 when the stage changes;
5. report only checks which ran.

Resume from the [R054 registry](docs/audit/ANALYSIS_REGISTRY.md#resume), not chat summaries or historical issues.
