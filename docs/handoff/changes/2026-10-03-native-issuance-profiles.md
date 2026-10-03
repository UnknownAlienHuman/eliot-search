# Contract change request — standalone native issuance profiles

**Status:** PROPOSED_REVIEW_PENDING. This request does not select or qualify a profile, assign an actor, issue a record, or authorize a package writer.

## Identity

- Requesting package: integration-owner tooling; no product package.
- Contract-owning package: integration-owner control plane.
- Base commit: `99db410fc540104928284f2ecd206b4b5baec8a0`.
- Architecture sections: none required for this request. Product standalone behavior already appears in `config/sections/instance.md` and `docs/handoff/W8_IMPLEMENTATION_PACKET.md`.

## Blocking problem

`swarm/schemas/types-v1.toml` requires `ImmutableArtifactRef.store_profile_ref` and `ImmutableSignatureRef.approval_profile_ref` to identify qualified profiles. The signature reference explicitly does not select a cryptographic algorithm without its profile. The normative issuance operations require actual artifact and signature readback before success.

The named control-plane read set defines the artifact-store and approval-reference shapes, but does not define artifact-store or approval-profile instance schemas, their qualification procedure, or a trusted actor-to-approval mapping. The separate context-manifest instance profile does not qualify either external profile. Supplying an opaque profile name, a self-consistent digest, or a local file cannot establish those properties. The advisory materialization planner checks reference shape and digest/actor equality; it is not an authoritative profile verifier.

ADR 0003, `docs/adr/0003-executable-swarm-orchestration-contract.md`, rejects a new orchestration service/database and permits Git commits plus append-only receipts. Neither that ADR nor the operation contract requires Eliot Governor or Memory OS. A native integration tool must remain usable without them.

The existing 4 MiB planner source cap is declared in `swarm/ticket-issuance-planner-v2.toml` and enforced by `GitTree::read_bytes`. It can bound the current advisory callers. It is not a declared global cap for future mutation records, operation receipts, approval artifacts, or profile instances.

## Producer and consumers

- Producer: integration owner provisions a profile and records its independent qualification evidence.
- Current consumers: advisory context/ticket planners consume references and must retain their non-authoritative status.
- Future consumers: native materialization, ticket/lease issuance, acknowledgement, submission, review, handoff, and recovery operations.
- Future compatibility surface: versioned profile definitions and immutable accepted qualification references, consumed by exact digest rather than a moving registry entry.

## Proposed contract

Resolve the following fields and rules before an authoritative adapter is implemented or selected:

1. **Artifact-store profile:** closed profile kind/version, one exact implementation owner, immutable locator grammar, object-byte limit, create-only publication, exact readback, cancellation/deadline behavior, unknown-outcome recovery, and the immutable qualification reference required for use.
2. **Approval profile:** closed profile kind/version, approval algorithm or explicitly specified authentication mechanism, domain-separated signed bytes, trusted actor mapping, verification material, revocation/currentness rules, finite artifact limits, and immutable qualification evidence. An opaque actor string or unsigned approval statement is insufficient.
3. **Actor selection:** distinguish profile definition from actual provisioned actor credentials and from writer/reviewer assignment. Materializer and reviewer remain distinct; credentials never appear in repository records, context artifacts, or reports.
4. **Record and operation budgets:** declare positive byte/count/deadline bounds at the applicable operation or profile. Existing per-source and context-renderer limits may be reused only for their declared consumers. No implicit unlimited value or unrelated global cap is introduced.
5. **Operation receipt and canonical input:** bind the exact input encoding/domain separator, operation ID, record/artifact publication identities, observed outcome, and recovery disposition. Specify the durable receipt shape and immutable storage before relying on it to reconcile a possible write.

The accepted operations contract already defines the mutation operations. Keep mutation disabled until the missing profile, qualification, actor-binding, record-status/nested-enum, and operation-receipt contracts are reviewed and accepted, and the selected profiles have independent qualification evidence. Implement the existing operations as a standalone local Rust integration tool, without Governor or Memory OS. This change request defines required contract fields; it does not choose profiles/defaults, provision credentials, qualify anything, or by itself permit mutation. It does not choose a cryptographic dependency, key, algorithm, approval artifact, or store location.

Pure byte, type, schema, and cross-record validation may be implemented before profile provisioning. Its result must state the verified scope and leave authority unresolved. It cannot be relabeled as signature authenticity, authoritative store readback, issued context, active lease, or package acceptance.

## Impact

- Security/access: prevents unverified profile names and actor references from granting writer authority; product access contracts are unchanged.
- Currentness/epoch/publication: create-only control publication and unknown-outcome recovery must retain exact readback requirements.
- Source identity/readback: source identity remains the immutable Git base/blob plus exact source and artifact byte digests.
- Residency/retention/purge: keep approval secrets outside Git and writer context; specify profile-owned retention before selection.
- Protocol/wire compatibility: integration metadata only; no Search provider protocol change.
- Migration/backward compatibility: zero issued records exist at the base. Advisory artifacts remain non-authoritative and are not silently promoted.
- Resource budget: existing advisory limits remain effective; missing mutation/profile budgets need explicit definitions.

## Tests

- Valid provisioned profile and exact readback; unknown profile/version/field rejected.
- Changed approval payload, actor, immutable context, key/authentication material, and revoked identity rejected.
- Same operation/input returns its prior durable result; changed input conflicts.
- Publication cancellation/fault before and after a possible write preserves the documented recovery disposition.
- Exact byte/count/deadline boundaries, including zero and over-limit inputs.
- Traversal, non-regular artifact objects, mutable references, and secret-bearing records rejected.
- Run the accepted operations with Governor and Memory OS absent; retain raw results and independent exact-commit review.

These are required future acceptance cases, not executed evidence or qualification results.

## Decision needed

**Compatible control-plane contract extension**, with an ADR if the selected profile or implementation choice requires one. The integration owner must first establish the closed profile/receipt contracts and obtain independent review. Actual profile selection, qualification, and actor provisioning remain separate steps with external authority inputs and executed evidence; they are not defaults to invent and this request cannot satisfy them.
