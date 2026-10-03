# ADR 0003 — Executable swarm orchestration contract

- **Status:** superseded by ADR 0005
- **Date:** 2026-08-29
- **Superseded:** 2026-10-03
- **Scope:** historical repository-local development coordination only
- **Architecture:** ELIOT Search 8.4

## Context

This ADR was introduced to make a large, concurrent repository refactor reproducible. It described
package assignments, bounded writer scopes, dependency handoffs and evidence records so multiple coding
agents would not edit the same package from incompatible bases.

The decision was never intended to define an ELIOT Search runtime capability. It explicitly excluded
product code, vendor selection and runtime authority, and rejected creating a new orchestration service
or database.

Subsequent repository instructions incorrectly promoted the development metadata into a mandatory
ticket, lease, signature, approval-profile and control-record system. Agents then implemented generic
swarm-controller machinery instead of completing the standalone Rust/Qdrant search product.

## Historical decision

The original decision permitted Git-tracked, repository-local planning metadata for coordinating a
specific development campaign. It did not require a running controller and did not grant ELIOT Search
any authority over agents, tasks, acceptance or completion.

Useful historical ideas remain valid as optional engineering practice:

- one active writer per overlapping package scope;
- exact source revision and bounded write scope;
- explicit dependency/API handoff before downstream integration;
- independent review and exact-head evidence;
- append-only audit history for accepted product changes.

## Supersession

ADR 0005 establishes the current boundary:

- ELIOT Search is a standalone search product and provider;
- generic agent orchestration belongs to `eliot-swarm-controller` and later ELIOT Memory OS;
- `swarm/**`, ticket drafts, launch-state files, leases, receipts and related tooling are advisory or
  historical repository metadata only;
- their absence or state cannot authorize, block or alter Search implementation or runtime behavior;
- no ticket/lease/signature/approval-profile implementation is required to work on Search;
- Search integration with ELIOT is through typed provider contracts, never through ownership of the
  Governor, WorkScope, tasks, agent roles, review acceptance or finish authority.

Any lower-precedence document that treats the old orchestration metadata as product or implementation
authority is superseded by ADR 0005.

## Consequences

- Existing product architecture and package ownership remain unchanged.
- The repository may keep small optional planning aids, but must not grow another controller.
- Generic orchestration schemas, issuers, credentials, role systems, mailboxes and schedulers are out of
  scope for this repository.
- Development proceeds from the architecture, accepted product contracts, the maintainer request and
  ordinary branch/worktree ownership.
- Historical commits remain in Git; superseded controller artifacts need not remain in the active tree.

## Rejected alternatives

- **Keep the mandatory ticket/lease system because it is already documented:** documentation cannot
  transfer another product's responsibility into Search.
- **Embed a reduced controller in `eliot-searchd`:** this would couple standalone search startup and
  maintenance to unrelated agent-management state.
- **Require ELIOT Memory OS for normal Search operation:** this violates the standalone product boundary.
- **Delete all development coordination practices:** exact scopes, handoffs and review remain useful as
  ordinary repository workflow without becoming a product control plane.
