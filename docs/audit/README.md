# ELIOT Search: audit and implementation entrypoint

**Audit base:** `1a6d17d073c5f6d1fe50f21082439f1965d24ef8`. **Product/release:** not qualified. **Next current coding task:** #256, unless the manager has since delivered it on a newer accepted base. Do not reset an active worktree to a dated audit SHA.

## Start here — the complete project, not just Wave 2

1. [Whole-project completion map](../product/PROJECT_COMPLETION.md): all stages through source, indexed retrieval, navigation, profiles, lifecycle, installed candidate, independent qualification and publication.
2. [Completion-audit findings and evidence boundary](PROJECT_COMPLETION_AUDIT_2026-10-09.md).
3. [Root instructions](../../AGENTS.md), [architecture](../architecture/README.md), accepted ADRs and nearest package instructions.
4. [Coordinator #97](https://github.com/UnknownAlienHuman/eliot-search/pull/97), assigned current issue and current source.
5. [Remaining Wave-2 packet](WAVE2_SINGLE_MANAGER_PACKET_2026-10-09.md) and [exact integration exceptions](WAVE2_PACKAGE_EXCEPTION_MATRIX_2026-10-09.md) for those specific source slices.

One manager owns one writer worktree, dependency pins, Cargo.lock and integration. Five to ten subagents inspect/research/review bounded assignments. Completed foundation tasks must not be restarted. Phase names such as `234.build` and `234.review` separate implementation from use; they are not new issues, runtime states or a controller system.

## Delivered implementation and recorded checks

- #317/#318 restored existing xtask gates.
- #237/#319 delivered canonical/digest foundation.
- #253/#322 delivered the full-fold decision and goldens, not the production tokenizer.
- #258/#323 delivered indexed contracts.
- #250/#326 delivered bounded metadata/status tooling.
- #324 remains a real validator rerun obligation; docs correction alone is not PASS.
- #327 remains a dependency/advisory obligation; #329 removes legacy identity APIs after consumer cutover.

[Canonical implementation](CANONICAL_FOUNDATION_IMPLEMENTATION_2026-10-09.md), [donor closure](CRYPTO_DONOR_ACCEPTANCE_2026-10-09.md), [indexed contract](../../crates/search-contracts/INDEXED_CONTRACT.md), [Cargo metadata cutover](WAVE2_CARGO_METADATA_CUTOVER_2026-10-09.md), [Wave-1/2 recheck](WAVE1_RECHECK_WAVE2_CONTINUATION_2026-10-09.md) and [package status](../product/PACKAGE_STATUS.toml) retain their exact evidence scope. Package execution-chain fields are obligation cross-references, not a replacement for the phase DAG or an assertion that a historical PR is executable.

## Technical findings and donors

[Master technical audit](ELIOT_SEARCH_MASTER_AUDIT_2026-10-09.md) and specialized source audits in this directory retain the F01–F154 obligation set. Older source snapshots, candidate versions and two-manager/parallel-writer launch text are historical. The new completion map routes findings to actual owners; it does not declare unfixed findings resolved.

[Wave-2 donor register](WAVE2_DONOR_ACCEPTANCE_2026-10-09.md) supplies exact earlier decisions; the whole-project map supplies later primary documentation and boundaries. Recheck exact versions/features/advisories when adopting or changing a dependency, not before every unrelated task. Do not copy a large framework when the useful part is a small library, standard or invariant.

## Noncircular completion

Configured is not Operational; Operational is not ReleaseQualified. Product candidate startup must not import or wait for its own future benchmark/attestation. Build the packaging/runner/verifier tools, freeze a candidate and plan, execute installed cases, independently verify raw subjects, then publish the exact tested bytes after human approval. Optional profiles remain individually declared and unfinished when not shipped.

## Evidence limit

No documentation or issue rewrite is a Cargo/Clippy/native/Qdrant/installed/scale/release PASS. Author-reported scoped checks remain scoped; failures, partial, unavailable and not-run results stay visible. This audit cutover changes no Rust, manifests, lockfile, workflows or product behavior. It does not claim a new full line-by-line code review or absence of all defects.

Historical `swarm/**`, handoff packets, old branch trees and Architecture Part II are archaeology/obligation records only. They cannot create a second source, client, catalog, journal, index, security or execution authority.