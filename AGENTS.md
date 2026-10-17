# ELIOT Search repository instructions

## Product and authority

ELIOT Search is a standalone, Qdrant-backed analysis and retrieval framework for agents and human
operators. It admits local sources, retains immutable revisions, prepares lexical/structural projections,
retrieves candidates, verifies exact source evidence and returns bounded navigation across repositories,
portfolios and qualified document corpora. No model is required in the hot path.

Maintainer instructions determine the authorized task and working method. For product semantics use:

1. Part I of [Architecture 8.4](docs/architecture/ELIOT_SEARCH_8.4_IMPLEMENTATION_MASTER.md);
2. accepted ADRs, especially [0005](docs/adr/0005-standalone-search-product-and-controller-boundary.md)
   and [0006](docs/adr/0006-agent-analysis-framework-product-scope.md);
3. accepted public contracts, provider schemas and qualification contracts;
4. nearest package `AGENTS.md`, `FUNCTIONS.md` and owned documentation.

Compiled source shows what exists, not whether it meets those contracts. Current maintainer handoff,
[#352](https://github.com/UnknownAlienHuman/eliot-search/issues/352), the actual active issue/PR and
merged source establish the writer queue. Refresh stale issue bodies instead of repeating delivered work.
Preserve the active manager and its WIP; do not reset or replace its branch from a dated map.

Part II, P00 scheduling, `swarm/**`, old launch packets, ticket/lease records and dated audits are
historical/advisory material. They cannot grant or block implementation authority. Search neither embeds
nor requires Memory OS, a Governor, swarm controller, tasks, scheduling, mailboxes, assignment leases,
review acceptance or finish authority. ELIOT integration is a leaf adapter over the standalone owners.

## Read only the relevant context

Before editing, read this file, the [execution protocol](docs/product/EXECUTION_PROTOCOL.md), the exact
current issue, nearest package instructions and the relevant Part I/contract sections. Use the
[documentation map](docs/README.md) to find them; do not reload the entire master or every dated packet
for each slice. Consult these only when relevant:

- [Verified audit and measured reduction candidates](docs/audit/ARCHITECTURAL_AND_CODE_AUDIT_2026-10-17.md).
- [Existing donor/reuse index](docs/audit/DONOR_VERIFICATION_REGISTER_2026-10-09.md).
- [Long-range product obligations](docs/product/PROJECT_COMPLETION.md).

None is an additional launch gate. A maintainer request or an exact ready issue plus the sole active
writer is sufficient authorization; an audit-wide closure is not a prerequisite for a coherent change.

## Refactor by removing duplicate ownership

1. **Reuse the existing owner and donor.** Cryptographic algorithms belong to `search-contracts` and
   accepted `blake3`/`sha2` dependencies; canonical CBOR belongs to its existing closed codec. Preserve
   the exact algorithm, raw/framed bytes, field order, bounds and profile. Homemade FNV/SplitMix/XOR
   semantic digests and fixture constants cannot mint production evidence. S12.2's qualified sparse
   token-to-bucket mapping is distinct from a cryptographic fingerprint.
2. **Cut over and delete together.** Name the state/effect owner, its immediate consumers, the replaced
   API/path and compatibility disposition. Remove the old writer/fallback with its last callers. A
   retained reader needs a concrete stored format, caller and deletion owner. Never rehash stored IDs
   in place or delete `sealed_*`, snapshots or plaintext readers by wildcard.
3. **Complete actual callers.** A dead facade or enabled feature is not integration. Bind required
   composition to real admission, native identity, security observations and effects before activation.
   A diagnostic file's parent is not an admitted root. Fixed Permitted/barrier/profile/vault values
   cannot substitute for observations. Remove unused synthetic wrappers; keep real fixtures test-only.
4. **Reduce mechanisms before rearranging files.** Delete obsolete prototypes, duplicated codecs,
   field tables and orchestration before considering package moves. Cargo boundaries need a real
   independent state/effect, security, dependency, replacement or lifecycle boundary. A capability cell
   is not automatically a crate. Do not create forwarding crates, a generic framework or a second
   implementation to meet a file/line quota; see [ADR 0001](docs/adr/0001-capability-cell-crate-decomposition.md).
5. **Measure the result honestly.** Report added/deleted code, remaining callers and retained fixtures
   for a reduction slice. File moves, minification, generated output and test deletion do not establish
   reduced product complexity. Existing size debt does not block a bounded shrinking change.
6. **Keep one record for each decision.** Update the existing issue, owner contract or document and
   remove contradictory scheduling text. Do not add new exemption/ledger/coverage machinery. Repair
   actual producers and remove resolved exemptions through the existing guard; unrelated classified
   sites in a touched file remain separate work. Preserve useful product validators while removing
   historical tooling under #214/#525.

## Working method

One manager owns one source worktree/PR and integrates manifests and `Cargo.lock`. Subagents are
read/research/review only, assigned narrow questions as useful; there is no minimum team size. An
explicit owner-and-immediate-consumer cutover supersedes generic package-only restrictions for those
named paths. Other package prohibitions still apply.

Use the [execution protocol](docs/product/EXECUTION_PROTOCOL.md) for ready fields, scope freeze by the
second source commit, B0/F1/F2/D/Q triage, review budgets and final-head review. Implement the product
change first. Run only the declared production check, strict Clippy and affected source guards; defer
test/native/fault/scale execution until the complete integrated implementation. Compilation is not
qualification. Record commands and outcomes exactly; never claim an unperformed run.

Successors start from newly merged `main`; no stacked source PRs or overlapping writers. Read-only
successor research may overlap final review. PROCESS/DOCS changes contain no product source, manifests,
lockfile or workflows. Keep Actions manual-only. Keep the PR body current instead of posting progress
comments after every commit. Do not commit local paths, credentials, source captures or unredacted logs.

## Product invariants

- Qdrant is the sole indexed retrieval backend: sparse/optional dense vectors, payload indexes,
  filtered retrieval/IDF, count/scroll and point readback. No second production search/index database.
- redb owns bounded technical control state, never a second searchable corpus.
- Qdrant payload is projection metadata. Citations require authoritative exact source/revision readback.
  Heuristics propose candidates; a complete negative claim requires an exact frozen denominator.
- Retrieval, IDF population, denominator, count/scroll, facets/groups/clusters, recommend/discover and
  validation share one eligibility base filter. Denied data cannot influence permitted ranking,
  visibility, statistics, suggestions or traces. Post-filtering alone does not enforce this boundary.
- Paths are locators. Native file, source, revision, representation and projection identities remain
  distinct. Immutable revisions/manifests are never rewritten in place.
- Publication is generation/epoch safe. Guarded control-state publication, not an alias change alone,
  establishes the visible route. Unknown external outcomes are neither success nor blindly replayable.
- Restrictive deny/purge/shadow changes fail closed across retrieval, handles and continuations.
- Portfolios are explicit immutable scopes. Forks/mirrors/copies collapse by lineage; they cannot
  masquerade as independent evidence. Cross-population raw scores are not directly comparable;
  fusion is deterministic and versioned.
- Query admission is read-only: no ordinary durable query job or second query-state store.
- Standalone daemon/CLI and optional ELIOT adapter share one state/query authority. DIRECT stays usable
  under Qdrant failure. Build features do not imply operational readiness or release qualification.
- Vendor Qdrant types stay inside `search-qdrant-bridge`; public ports use Eliot-owned types.
- Search never executes indexed code, document macros or remote resources. Optional model/document
  workers remain disabled until separately qualified; lexical/code baseline does not require them.
- Preserve the eleven v1 recipes. Free-text retrieval/orientation use the versioned #213 extension,
  never overloaded `find_text@1` or a raw Qdrant interface.
- Large repositories/portfolios require bounded legs, budgets, progressive output and measured #215
  qualification. Document profiles share existing source/map/query owners and require #216 evidence.
- Destructive lifecycle actions require complete bounded reachability and actual owner release across
  live data, pins, operations, backup/restore, rollback copies and keys; a count or receipt is insufficient.

## Evidence and external writes

Exact candidate qualification must cover native Windows identity/containment/secrets/pipes, real Qdrant
artifact/schema/query/restart behavior, source-to-validated-result integration, access/unknown-outcome/
recovery, scale/resources and every shipped document or adapter profile. Installed evidence precedes
release. Source presence, mock success, issue status, signatures and review badges cannot replace it.

Before concluding the GitHub connector is read-only, inspect its full catalog, repository
`permissions.push` and available API writes. An unattached blob may be a harmless write probe. Missing
local Git credentials or a filtered tool list does not establish read-only access.
