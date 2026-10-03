# ADR 0005 — Standalone Search product and controller boundary

- **Status:** accepted
- **Date:** 2026-10-03
- **Scope:** product ownership, standalone operation and cross-repository integration
- **Architecture:** ELIOT Search 8.4
- **Supersedes:** ADR 0003 as implementation authority

## Context

ELIOT Search is the local source preparation and retrieval provider for ELIOT, but it is also a
standalone search product. Its product architecture is Rust around Qdrant, with redb for technical
control state and scoped immutable storage for source/preparation evidence.

The ELIOT ecosystem has separate owners for agent execution and canonical task authority:

- `eliot-swarm-controller` is the current standalone prototype for native coding-agent orchestration;
- `eliot-memory-os` owns Principal, WorkScope, tasks, authority, canonical history, Context Compiler,
  Governor, verification and finish semantics;
- `eliot-search` owns source preparation, indexing, retrieval, candidate validation and typed provider
  responses.

Repository-local coordination documents were incorrectly expanded into ticket issuance, writer leases,
approval/signature profiles and generic control-record tooling inside Search. Those mechanisms did not
advance the Qdrant product spine and duplicated responsibilities owned by the controller/Memory OS.

## Decision

1. **Standalone is mandatory.** `eliot-searchd` and `eliot-search` must be installable, startable and
   useful without ELIOT Memory OS, a Governor, a swarm controller or any agent-management service.
2. **Qdrant remains the only indexed search database.** redb is technical control state, and immutable
   source/preparation evidence remains outside Qdrant. No second searchable database is introduced.
3. **ELIOT integration is an optional typed edge.** Search accepts bounded provider requests and returns
   candidates, coverage, freshness, assurance, reason codes and opaque handles through accepted
   contracts. The standalone API and the ELIOT adapter use the same underlying product owners.
4. **Search does not own agent authority.** It must not implement or persist generic Tasks, WorkScopes,
   Attempts, agent roles, GM/Governor state, mailboxes, schedulers, writer leases, assignment tickets,
   review acceptance, finish authority or model-harness lifecycle.
5. **External controller responsibility stays external.** Reusable agent orchestration belongs in
   `eliot-swarm-controller` and, after integration, `eliot-memory-os`. Search may be indexed by or called
   from those products; it does not embed them.
6. **Repository coordination is advisory.** Files under `swarm/**`, `docs/handoff/**` and historical
   `docs/execution/**` may describe package boundaries, plans and past evidence. They are not runtime
   inputs and cannot authorize or block implementation.
7. **Normal repository work needs no issuance protocol.** A maintainer request, issue or PR plus one
   non-overlapping branch/worktree is sufficient. No ticket, lease, acknowledgement, signature profile
   or control-plane receipt is required before editing Search.
8. **Product evidence remains strict.** Source presence, a planning record or a compile does not qualify
   a capability. Qdrant, Windows security, provider transport and end-to-end claims still require their
   exact product qualification evidence.
9. **The Search adapter remains a leaf.** `search-eliot-adapter` may translate accepted provider
   contracts. It may not import ELIOT task authority or make Search startup depend on ELIOT.
10. **Boundary violations are removed, not completed.** Generic issuance/lease/signature/controller
    implementations added to Search are out of scope even when internally consistent or well tested.

## Required repository behavior

- Root and package instructions must prioritize the Search architecture and product backlog.
- Lower-precedence documents that say `launch-state.toml`, a ticket or a lease is required for coding are
  historical and non-authoritative.
- Tools may validate product contracts, package boundaries, Qdrant isolation and qualification inputs.
  They must not become a generic orchestrator or credential/approval authority.
- Build and release artifacts must not include swarm-controller metadata or require it at startup.
- Standalone and ELIOT-integrated modes must share the same source, access, publication and retrieval
  truth; integration cannot create a second authority path.

## Consequences

- Work returns to the real completion path: durable source/control state, real Qdrant projection and
  retrieval, provider/CLI composition, currentness, recovery and qualification.
- ELIOT can consume Search without owning its internals, while non-ELIOT users can run the same search
  engine independently.
- Generic controller improvements can proceed in the dedicated controller repository without bloating
  Search or coupling its release schedule.
- Historical orchestration commits remain discoverable in Git, but the active tree presents only
  product-relevant instructions and optional lightweight development metadata.

## Rejected alternatives

- **Make Memory OS the mandatory Search host:** prevents standalone deployment and creates reverse
  authority from a provider into its consumer.
- **Keep a private Search-specific swarm controller:** duplicates the dedicated controller and produces
  incompatible task/receipt semantics.
- **Treat repository tickets as harmless documentation while making agents obey them:** this still blocks
  product work and recreates the control plane operationally.
- **Remove the ELIOT adapter:** unnecessary; typed optional integration is part of the product boundary.
