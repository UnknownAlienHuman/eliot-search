# Gate evidence catalog

`swarm/gates.toml` names the evidence required by each delivery gate. This document defines what an
evidence reference must contain and what it is allowed to prove.

## Evidence record

Every evidence reference records:

```yaml
EvidenceRef:
  evidence_id: stable_registry_id
  producer_package_or_integration_owner: string
  repository_commit: git_sha
  dependency_api_digests: [digest]
  command_or_fixture: string
  environment:
    operating_system: string
    architecture: string
    toolchain: string
    external_artifacts: [name_version_digest]
  result: PASS | FAIL | UNAVAILABLE
  raw_output_ref: immutable_ref
  generated_at: timestamp
  reviewer: opaque_identity
  notes: bounded_non_content_metadata
```

A prose statement without command/fixture identity and immutable raw-output reference is not gate
evidence. `UNAVAILABLE` is truthful state, not a pass.

## Evidence classes

| Class | Proves | Does not prove |
|---|---|---|
| contract | schema, serialization, type and pure invariant behavior | runtime I/O or performance |
| dependency policy | source, version, license and graph policy | vendor runtime safety |
| deterministic fixture | behavior against a pinned input/artifact | broad production quality |
| fault proof | invariant preservation at named failpoints | untested failpoints/platforms |
| security/noninterference | absence of a named leakage/authority path in tested conditions | universal security |
| qualification | exact OS/vendor artifact behavior | another version or platform |
| performance/resource | measured corpus and machine result | a general SLA |
| product pulse | comparative acceptance decision with raw evidence | architectural correctness by itself |

## Gate summaries

### G0 / P00 — Contract

Requires the architecture digest challenge, workspace/assignment/registry parity, exact recipe set,
epoch/sentinel rules, canonical schema fixtures, dependency direction, dependency/license policy and
pure contract/domain tests.

No Qdrant, redb, filesystem, Windows containment or performance claim can be satisfied by G0.

### G1 / P01–P04 — Direct

Requires single-owner lifecycle, bounded journal behavior, no-write hot query admission, source
admission/identity, no-execute stable reads, residency-aware immutable revisions, anchors and exact
readback without Qdrant.

### G2 / P05–P08 — Lexical

Requires exact Qdrant artifact/process qualification, capability/schema fixtures, lexical golden
vectors, point-collision protection, serialized publication fault matrix, pin/reclaim proofs, access
noninterference and bounded lexical result cards.

### G3 / P09–P12 — Code and exact proof

Requires observation-gap handling, exhaustive unsaved-byte non-persistence, Rust structure assurance,
subject ambiguity behavior, comparison fixtures and complete-scope exact-proof failure on drift,
unreadable items or cancellation.

### G4 / P14 — Generic client edge

Requires framing/replay/cancellation limits, authenticated binding and grant checks, capability
descriptor filtering, handle expansion authorization and generic request→plan→candidate round trip.
Optional ELIOT/Research profiles contribute evidence only when explicitly enabled.

### G5 / P15 — Product acceptance

Requires raw A/B/C control-corpus results, latency/resource measurements, recovery matrix,
source-admission/leakage audit, protocol stress and an explicit accepted/rejected product verdict.
Green unit tests alone cannot satisfy G5.

### G6 / P16–P18 — Optional depth

Requires an accepted G5 receipt, a dedicated ADR, exact provider/artifact qualification, measured
material benefit and uninstall/removal fallback. Optional depth never retroactively becomes baseline.

## Ownership

Package-local evidence is produced by the package owner and reviewed independently. Cross-package and
wave evidence is assembled by the integration owner. Product Pulse evidence is owned by `search-eval`.
No package writer may mark its own wave accepted.
