# Working on ELIOT Search

Start with [docs/README.md](docs/README.md), then read the
[architecture](docs/architecture/README.md), the
[implementation spine](docs/implementation/README.md), and only the nearest owner recipe needed for the current
vertical cut.

## One manager, one vertical cut

Use one manager-owned worktree. The manager owns the integration boundary and commit. Parallel workers may inspect
or implement disjoint files, but they do not invent competing interfaces or merge overlapping owner changes.

Before code, name:

1. the real entrypoint;
2. the concrete owner/effect being changed;
3. the immediate real consumer;
4. the obsolete path removed by the same cut;
5. the smallest Rust 1.98 `check`/Clippy closure.

Then implement that path. Do not restart a repository-wide audit, create another control plane, or block code on
historical packets, receipts, package counts, broad tests, optional donors, or unrelated open issues.

## Baseline is deliberately unblocked

The first working product uses:

- one parsed invocation and small typed settings;
- one current-format root guard;
- the **plain** immutable-object profile;
- one explicitly selected file;
- one retained source head and one preparation result;
- one fresh Qdrant generation;
- one one-shot Search request;
- one pinned evidence lookup and CLI result.

The encrypted object profile is optional. #598 owns that profile only after an accepted public handoff; it is not
a prerequisite for the plain baseline. Do not inspect or modify #598's private branch/worktrees.

Configuration grows with consumers. Slice 1 implements only invocation/root/plain-storage settings. Source,
Qdrant and query fields are added in the slice that constructs their real owner. Do not create speculative
settings, adapters or readiness flags.

## Publication safety without a recovery operating system

New Qdrant points are immutable and created with insert-only semantics. A retry carries the same fixed point ID
and bytes. Closing a point writes the same fixed `valid_until` boundary. The baseline never physically deletes
individual points, so a delayed duplicate create cannot resurrect a deleted ID. Compaction/rebuild creates a
never-reused generation; cleanup deletes only whole retired generations after readers release them.

Do not reintroduce per-operation ledgers, isolated-route succession, point-deletion receipts or a second
publication state machine.

## Authority and verification

Architecture owns user-visible requirements. Implementation owns delivery order and cross-owner invariants.
Nearest `FUNCTIONS.md` owns exact seams. The registry records evidence and unrun work; it is not permission.

After production code, run the smallest locked Rust 1.98 compiler/Clippy closure for changed owners and immediate
consumers. Focused runtime cases follow when the path exists. Record only commands that actually ran.
