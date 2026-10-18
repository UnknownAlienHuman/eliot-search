# ELIOT Search repository instructions

## Product and current decisions

ELIOT Search is a standalone Qdrant-backed analysis/retrieval tool for agents and human operators.
Its value is useful navigation and exact source evidence across explicit local repositories/corpora.
No model is required in the hot path; agent/task/controller orchestration is outside this product.

Maintainer instructions govern the task. Read the [current architecture](docs/architecture/README.md)
first. Its 2026-10-18 decisions replace the listed 8.4 requirements for the local baseline. The old
master, capability tables, package contracts and dated issues cannot reinstate retired compatibility,
multi-domain, encryption, receipt or process obligations. Read older sections only for behavior actually
being retained. Update the touched owner's guidance with its implementation.

[ADR 0006](docs/adr/0006-agent-analysis-framework-product-scope.md) preserves the analysis/large-corpus
mission. Current source shows what exists; a facade or closed issue does not prove integration.
Use [#352](https://github.com/UnknownAlienHuman/eliot-search/issues/352) and the actual handoff to avoid
overlapping writers. Preserve the active #598 manager, private branch and unpublished work.

## Read the code before assigning more code

Read the actual entrypoint, caller, owner and immediate consumers. Use the
[audit](docs/audit/ARCHITECTURAL_AND_CODE_AUDIT_2026-10-17.md),
[donors](docs/audit/DONOR_VERIFICATION_REGISTER_2026-10-09.md) and
[completion map](docs/product/PROJECT_COMPLETION.md) to find them, not as another task queue.
Check whether the requested mechanism already exists: the provider client, blocking Qdrant runtime,
S11/planner and evidence/orientation schemas have already been implemented.

Prefer a concrete source → preparation → Qdrant → validated-result change. Delete its replaced path
and last consumers in the same change. Do not deliver another uncalled constructor, handler registry,
receipt producer or generic port framework while the useful endpoint remains absent.

## Simplification rules

- Remove mechanisms before moving files. A package needs a real dependency/runtime/replacement boundary;
  a capability cell or agent assignment does not need a crate. Call pure code directly.
- Keep one state/effect owner. Use existing redb, Qdrant, parser, matching, serialization and crypto
  dependencies. Do not copy algorithms or create a facade around each library call.
- Pre-release compatibility is not automatic. Default to a new format/generation and rebuild derived
  state. Preserve originals, sole retained revisions, keys and explicit configuration; leave old roots
  untouched. Keep an old reader only for concrete retained data or an external consumer. No automatic
  plaintext downgrade or in-place rehash/relabel of stored identities.
- Delete unnecessary hashes/receipts with their unused consumers. Where identity/integrity is needed,
  use the real shared BLAKE3/SHA-256 owner and exact bytes. A fixture constant or FNV value cannot stand
  in for a cryptographic digest. Sparse token bucket mapping is a separate noncryptographic role.
- Retire per-site source-hash/issue-state ledgers, committed coverage output and source-shape assertions
  as merge requirements. Keep useful behavioral and dependency checks. Do not build another exemption
  catalogue, waiver process or controller to replace them.
- A reduction removes behavior/state/callers. Moves, minification, forwarding modules and indiscriminate
  test deletion are not product simplification. Report remaining work without an invented LOC target.

## Product behavior to retain

- Qdrant is the only indexed retrieval backend; redb is bounded control state.
- Reopen exact source/revision bytes for citations. A path, score or Qdrant payload is not source truth.
- Use explicit scope and one coherent route/epoch. Eligibility also governs IDF and any requested
  count/statistics. Ordinary top-k is non-exhaustive and does not require an exact denominator.
- Commit publication in control state after real effects/readback. A possible external write followed
  by timeout is unknown, not rolled back. Keep active pins and actual restrictive source removal.
- Use an OS-owned local connection, private Qdrant credential, contained reads, finite resource limits
  and one absolute request deadline. Do not execute indexed code/macros/remote resources.
- Keep DIRECT usable and accurately named when indexed retrieval is unavailable. Capabilities reflect
  connected handlers. Optional encryption/delegation/exports/models are not startup prerequisites.
- Keep vendor types inside their adapter. Return useful bounded locations, snippets and errors within
  the user's scope; exclude credentials and source/query bodies from ordinary logs.

## Work and verification

One manager writes and integrates; subagents research/review narrow questions. Use the
[execution protocol](docs/product/EXECUTION_PROTOCOL.md). Describe the concrete result, consumers,
removed mechanism and necessary verification in the existing PR. There is no mandatory scope form,
second-commit freeze, classification vocabulary or formal-review badge gate.

Implement first, then check changed production code and consumers with pinned Rust/Clippy. Execute
relevant behavior cases after the integrated implementation exists. Compare a failing baseline only
when needed to distinguish a regression. Do not run or repair obsolete whole-workspace harnesses merely
to obtain a badge. Native/live-Qdrant/performance evidence is required for the claims being made.

Keep the existing preparation PR documentation-only; product changes belong to the active manager's
source work. Keep Actions manual-only. Do not commit credentials, private source captures or local logs.
Update PR/issue bodies instead of posting progress comments. No source presence, receipt, source hash or
review badge substitutes for working behavior.
