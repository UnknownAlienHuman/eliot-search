# Implementation waves

The architecture authorizes P00 first. `swarm/launch-state.toml` is the only current launch authority;
this document defines the future dependency-safe sequence.

## W0 — Contract freeze

1. `search-contracts` implements the exact P00 field-level pack.
2. Integration owner accepts its API/schema digest and resolves every contract challenge.
3. `search-domain` and `search-ports` may then run in parallel against that immutable handoff.
4. Integration owner pins the real Windows-compatible toolchain/dependencies, generates `Cargo.lock`,
   runs P00 policy/tests and publishes the W0 receipt.

No W1 package starts before that receipt.

## W1 — Process and control shell

Runtime owner, OS-bound secret adapter, bounded redb journal, generic frame/session shell and thin
CLI/daemon composition. Proves one-root ownership, secret non-disclosure, read-only hot admission,
framing limits and clean shutdown.

## W2 — Direct source spine

Source admission, identity/path history, registry/ownership, stable no-execute reads, residency-aware
revision CAS, text/code materialization and deterministic unitization. No index is required.

## W3 — Qualified lexical index

Exact Qdrant process artifact and containment, data-plane capability/schema gate, lexical fixtures,
point identity/projection manifests, linearizable publication, epoch pins and exact retired-point
reclamation.

## W4 — Baseline query product

Pre-candidate access, server-owned plans, bounded leg execution, exact source-backed validation, handle
state, compact cards, continuations and raw read/grep evaluation baseline.

## W5 — Current workspace and code structure

Observation reconciliation, saved/unsaved overlays and qualified Rust structural enrichment.

## W6 — Comparison and exact proof

Ambiguity-preserving subject resolution, descriptive cross-repository comparison and frozen-denominator
exact execution reports.

## W7 — Security and lifecycle hardening

Restrictive-revocation linearization, durable handles, CAS mark-and-sweep, purge receipts/tombstones,
restore quarantine and ordinary-reclaim/purge separation.

## W8 — Generic client edge

Full binding/capability/evidence edge. Optional ELIOT and Research profiles remain leaf packages,
disabled unless explicitly enabled and separately accepted.

## W9 — Product Pulse / Windows qualification

Control corpus A/B/C comparison, latency/resource/fault/security evidence and explicit product verdict.
Compilation or unit tests alone do not pass.

## W10 — Optional depth

Semantic, rerank, document or scale profiles only after accepted P15, dedicated ADR, exact artifact
qualification and measured material benefit.

## Launch rule

For each package the orchestrator verifies `swarm/crates.toml`, launch state, accepted dependency API
and port digests, creates one isolated worktree, provides only the bounded read set, rejects writes
outside package scope and merges in topological order.
