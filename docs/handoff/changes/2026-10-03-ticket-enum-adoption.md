# Ticket obligation enum integration decision

**State: ACCEPTED_INTEGRATION_CONTRACT.** The integration owner accepts the finite vocabulary subset
independently reviewed against `cac9c437ae928051b6e8a0824c11718d50a573f3`. Implementation and executed
conformance remain separate. This decision creates no ticket, actor, lease, qualification or handoff.

The source proposal remains a historical non-normative candidate at
`swarm/ticket-obligation-classes-v1.toml`. Only the four domains and nine field bindings below are adopted.

| Registered type | Exact ordered allowlist | Bound fields |
| --- | --- | --- |
| `FixtureQualificationStatus` | `FAILED`, `QUALIFIED`, `UNAVAILABLE` | `OrderedFixtureRef.qualification_status` |
| `NormalProcessExitClass` | `EXIT_NONZERO`, `EXIT_ZERO` | `BoundedCommandSpec.expected_exit_class`; `OrderedRawCommandOutcomeRef.exit_class` |
| `EvidenceArtifactClass` | `PACKAGE_HANDOFF_CANDIDATE`, `PUBLIC_API_SCHEMA_DIGEST`, `QUALIFICATION_PROBE_RESULT`, `RESIDUAL_RISK_RECORD`, `TEST_RESULT` | `BoundedCommandSpec.evidence_class`; `EvidenceRequirement.evidence_class`; `OrderedEvidenceRef.evidence_class`; `OrderedAcceptedEvidenceRef.evidence_class` |
| `ExpectedBehaviorClass` | `FAILURE`, `POLICY`, `RECOVERY`, `SUCCESS` | `EvidenceRequirement.acceptance_class`; `OrderedEvidenceRef.acceptance_class` |

Each type is a canonical, case-sensitive string enum with the exact `allowed` array above. Unknown
values and aliases are rejected. Replace only the nine corresponding `*_is_ClosedEnum` rules in
`swarm/schemas/types-v1.toml` with bindings to the declared named types. Preserve every existing
record's `canonical_fields` array and all other field rules. Keep type-registry format version 2 and
record-schema version 1: the existing `ClosedEnum` contract already requires exact allowed values,
and this completion changes no record shape. Update the control-plane registered-type count from
47 to 51 and its structural consumers; the schema-only disposition and zero issued records remain.

`QUALIFIED` requires immutable evidence for the exact fixture digest and independent review in the
owning registry; the token establishes neither. Preserve an observed normal exit even when it differs
from the command's expectation. No known normal termination is unavailable, not `EXIT_NONZERO`.
Expected behavior classes are separate from actual `PASS`, `FAIL` and `UNAVAILABLE` verdicts. Evidence
classes name artifacts and do not accept them. For an accepted evidence row, resolve its requirement
through the immutable handoff/submission/ticket chain; do not invent an `acceptance_class` output field.

The five input fields are necessary to validate a complete assignment ticket under the required
fixture/command/evidence sets in `TICKET_ISSUANCE_OPERATIONS.md` section 7. Their reused output bindings
can use the same vocabulary without changing field order or observation semantics. The independent
review found their source schemas unchanged from the proposal's pinned base `24ab0876ecff1bbf990a5ed140ee72403e865187`.

The proposed producer-command link, unavailable-check command link, and exactly-once coverage of all
planned commands are **not adopted**. The reviewed operation sections 7 and 9, submission fields
`command_outcomes[]` and `unavailable_checks[]`, and qualification probe F034 do not establish those
extra links or that coverage rule. They remain proposals requiring their own contract decision;
absence of the proposed links is not a new issuance precondition. Existing complete-evidence and
explicit-unavailable requirements remain effective, and no association is inferred from array order.

The output-only enums `OrderedUnavailableCheck.status` and `OrderedAcceptedEvidenceRef.availability`,
and the output representation for `artifact_required=false`, remain unresolved for operations that
must emit them. They do not block assignment-ticket issuance by themselves. Actor/store/approval
trust, acknowledgement encoding, canonical instance validation and executed qualification remain
separate real prerequisites.

Implementation ownership is integration-only: the type/control registries, their PowerShell checker,
and bounded native Rust validators/tests in `xtask`. A pure enum-binding/value validator must state
`NON_AUTHORITATIVE`, enforce finite input limits, and reject missing/duplicate/wrong bindings and
unknown values. It must not claim fixture qualification, process execution, signature verification,
store readback or issuance. No product-package edits are authorized by this integration correction.

The bounded implementation is integrated at `82315138f337ecbcab5ccb1a3f01b67330930572` after
independent review of exact candidate `b585f835a941a12e1db6afa2dc1d14ccdfb3ec54`. The
[execution record](../../execution/2026-10-03/TICKET_ENUM_BINDINGS_REVIEW.md) preserves 18/18 focused
tests, structural exit 0, original raw captures and earlier failures. These results do not complete
canonical instance validation or any authority/qualification prerequisite above.
