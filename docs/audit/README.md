# ELIOT Search audit entrypoint

**Verified implementation snapshot:** `17383079d316cc090eae04912534e2d16e6dc79d`.  
**Wave 1:** delivered by #319/#322.  
**Wave 2:** in progress; #258/#323 and #250/#326 delivered. Next: **#256**.  
**Product/release:** not qualified.  
**Coordinator:** [#97](https://github.com/UnknownAlienHuman/eliot-search/pull/97); use its latest accepted-base comment rather than a historical SHA in an audit.

## Execution entrypoints

1. [Current launch gate](AGENT_LAUNCH_GATE_2026-10-09.md).
2. [Remaining Wave-2 work and subagent map](WAVE2_SINGLE_MANAGER_PACKET_2026-10-09.md).
3. [Exact package integration exceptions](WAVE2_PACKAGE_EXCEPTION_MATRIX_2026-10-09.md).
4. The assigned current issue, nearest package instructions and actual source.

One manager owns integration, dependency pins, Cargo.lock and merge decisions. The original Wave-1 packet is now a delivery checklist/history, not an instruction to repeat #237.

## Review and source evidence

- [Wave-1 recheck and Wave-2 continuation review](WAVE1_RECHECK_WAVE2_CONTINUATION_2026-10-09.md).
- [Canonical foundation implementation](CANONICAL_FOUNDATION_IMPLEMENTATION_2026-10-09.md).
- [Accepted crypto/AST donor closure](CRYPTO_DONOR_ACCEPTANCE_2026-10-09.md).
- [Merged indexed contract](../../crates/search-contracts/INDEXED_CONTRACT.md).
- [Cargo metadata cutover](WAVE2_CARGO_METADATA_CUTOVER_2026-10-09.md).
- [Wave-2 donor mechanisms and accepted updates](WAVE2_DONOR_ACCEPTANCE_2026-10-09.md).
- [Machine-readable package status](../product/PACKAGE_STATUS.toml).

## Technical history

[Master audit](ELIOT_SEARCH_MASTER_AUDIT_2026-10-09.md) and the specialized source audits retain findings and rationale. Their older state snapshots, two-manager wording, now-completed prerequisites and old candidate pins are historical. They do not override the current launch gate, root/package instructions or accepted issue scope.

Older `swarm/**`, `docs/handoff/**`, packet branches and Architecture Part II are obligation/history material. They cannot create a second execution authority or require controller machinery inside standalone Search.

## Evidence boundary

A recorded scoped compiler/Clippy run is not a whole-workspace PASS. A source-classification ledger is not proof of a runtime digest's semantics. A merged PR or security-review badge is not native Windows, live Qdrant, installed-product, scale, disclosure or release qualification. Keep failed, partial, unavailable and not-run results visible.
