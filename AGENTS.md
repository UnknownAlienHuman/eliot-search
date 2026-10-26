# Working on ELIOT Search

Start with [docs/README.md](docs/README.md), then the
[architecture](docs/architecture/README.md), the
[implementation spine](docs/implementation/README.md), and only the nearest owner recipe needed for the current
vertical cut.

Canonical documentation owns current direction. Issues track responsibility and acceptance; they are not an
independent architecture or prerequisite graph. Historical packets, waves, receipts and immutable audit links are
evidence only.

## One manager, one vertical cut

Use one manager-owned worktree. The manager owns the integration boundary, interface decisions and commit.
Parallel workers may inspect or change disjoint files, but must not invent overlapping interfaces, merge their own
branches or create a second implementation route.

Before code, name:

1. real entrypoint;
2. concrete owner/effect;
3. immediate real consumer;
4. obsolete route removed by the cut or the next explicitly bounded cut;
5. smallest locked Rust 1.98 `check` and Clippy closure.

Then implement. Do not restart a repository-wide audit, create another control plane or block code on package
counts, optional donors, broad tests, unrelated issues or private work.

## Current delivery order

```text
1 Foundation
→ 2 one file / plain ObjectRef / preparation
    / Qdrant 1.20 scoring full-text generation / TextQuery / CLI excerpt
→ 3A restart + authoritative inventory + complete fresh-generation convergence
    + measured release gate
→ 3B point-level incrementality only where 3A budgets fail
→ 4 Exact/live session
→ 5 maintenance and optional depth
```

Foundation ships only invocation, root and plain-storage settings. Source, Qdrant and query settings arrive with
their real consumers. Plain storage is baseline; #598 encryption is optional.

## Platform publication rule

On Windows, root/object finalization must be tied to the opened stage handle and retained verified destination
parent. The current `tempfile::persist_noclobber` implementation is path-based `MoveFileExW`; do not use it as the
trusted finalizer. Implement and execute one owner-private handle-based canary. Do not create a generic filesystem
abstraction or silently fall back to path-only rename.

On Unix, use an owner-private no-replace directory-relative primitive or a verified same-filesystem link/unlink
fallback. Existing final objects are always opened and verified exactly.

## Qdrant baseline

The implementation cut upgrades server/client/schema/query together to Qdrant 1.20.x:

- retrieval text is stored once in payload;
- one scoring full-text index uses explicit text processing and the `code` tokenizer;
- query uses `TextQuery`;
- baseline sparse BM25 vector inference and fixed `avg_len` are removed;
- Exact remains independent of tokenization;
- `PublishedView` names the exact physical collection; aliases are not serving authority.

Stage 3A builds complete immutable physical generations. It is a release mode only for corpus classes whose
numeric latency/throughput/disk budgets pass. If a target class fails, Stage 3B incrementality is required before
release. Do not invent thresholds inside code; #215 records the product budgets and raw results.

Do not add custom per-root shards as a generic optimization: BM25 statistics are shard-local and cross-shard
portfolio ranking would require another fusion owner. Do not add Qdrant Edge as a fallback. It may be evaluated in
one isolated post-baseline spike and must replace the bridge cleanly if selected.

## Lifetime and lock discipline

- `DataRootGuard` is the outer process lifetime, not a serializable lease.
- Publication has one logical owner/task. No redb writer or view-manager mutex spans object/Qdrant I/O.
- Query pins one `PublishedView` under a short lock and releases it before backend/evidence work.
- Redb writers contain bounded metadata compare/commit only.
- Query reserves finite capacity before retrieval and mutates a window/cursor only after complete bounded output.
- Last guard drop wakes cleanup; it never deletes inline.

Follow the exact order in the implementation spine. Remove or adapt an old API that requires a reverse order.

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

Resume from the [R049 registry](docs/audit/ANALYSIS_REGISTRY.md#resume), not chat summaries, historical issue
bodies or default-branch code search.
