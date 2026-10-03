# Ticket obligation class proposal

**State: `PROPOSED_REVIEW_PENDING`.** This is a review candidate for five open `ClosedEnum` fields. It does not change `types-v1`, the assignment-ticket schema, any qualification registry, or issuance behavior. The machine-readable proposal is [`swarm/ticket-obligation-classes-v1.toml`](../../../swarm/ticket-obligation-classes-v1.toml).

The current schemas name the fields but do not declare their allowed values. The proposed values are deliberately separated into fixture qualification status, normal process exit class, evidence artifact kind, and expected behavior class. Qualification verdicts (`PASS`, `FAIL`, `UNAVAILABLE`) remain separate values in the ticket-issuance qualification profile. Every proposed enum token remains pending independent integration review.

| Field path | Proposed finite values | Intended meaning |
|---|---|---|
| `OrderedFixtureRef.qualification_status` | `FAILED`, `QUALIFIED`, `UNAVAILABLE` | Qualification state for the exact fixture digest. Not-run remains `UNAVAILABLE`; only immutable evidence plus the owning registry's independent review can support `QUALIFIED`. |
| `BoundedCommandSpec.expected_exit_class` | `EXIT_NONZERO`, `EXIT_ZERO` | Expected normal process termination. Does not predict whether a negative test is correct. |
| `BoundedCommandSpec.evidence_class`; `EvidenceRequirement.evidence_class` | `PACKAGE_HANDOFF_CANDIDATE`, `PUBLIC_API_SCHEMA_DIGEST`, `QUALIFICATION_PROBE_RESULT`, `RESIDUAL_RISK_RECORD`, `TEST_RESULT` | Semantic artifact kind. Raw command output remains its separate `OrderedRawCommandOutcomeRef`. |
| `EvidenceRequirement.acceptance_class` | `FAILURE`, `POLICY`, `RECOVERY`, `SUCCESS` | Expected behavior family, using the ticket-issuance qualification packet's `expected_classes` vocabulary. It is not a verdict or acceptance receipt. |

The reused output records should preserve these same domains: `OrderedRawCommandOutcomeRef.exit_class` uses the proposed process-exit classes; `OrderedEvidenceRef` copies the requirement's evidence and acceptance classes; and `OrderedAcceptedEvidenceRef.evidence_class` copies the referenced requirement's evidence class. The latter's `requirement_id` must resolve through the immutable ticket. The assignment-ticket schema currently has no `producer_command_id` or equivalent link from a command spec to an evidence requirement, so no producer relationship may be inferred from array order. Exact command/evidence class matching requires that relationship to be made explicit.

`FAILURE` means the tested behavior is an expected typed rejection; it does not mean the evidence record itself failed. Its exact `ClosedReasonCode` must come from the immutable fixture/probe contract. A matching negative test may have a `PASS` conformance verdict. Likewise, `RECOVERY` requires the exact registered disposition, while `POLICY` names a policy invariant. The actual verdict remains `PASS`, `FAIL`, or `UNAVAILABLE` as defined by the qualification packet.

Unavailability stays visible. A fixture whose qualification has not run is `UNAVAILABLE`. A command that does not start, times out, is cancelled, or completes with an unknown outcome cannot be rewritten as `EXIT_NONZERO`; it needs an explicit unavailable record and a semantically applicable typed reason. If the current `ClosedReasonCode` registry has no accurate reason, a separate contract change is required. This proposal does not add such a reason.

The P00 draft names fifteen test evidence obligations and six output categories. The proposed `TEST_RESULT`, `PUBLIC_API_SCHEMA_DIGEST`, `PACKAGE_HANDOFF_CANDIDATE`, and `RESIDUAL_RISK_RECORD` values correspond to those obligations or outputs; `QUALIFICATION_PROBE_RESULT` is grounded in the separate qualification packet. `PACKAGE_HANDOFF_CANDIDATE` does not imply an accepted handoff. Raw command outcomes remain represented by their dedicated ordered record, not by a new evidence class.

No blanket PASS rule is introduced. The ticket-issuance qualification profile requires all of its own mandatory probes to pass, and P00 G0 separately requires PASS, immutable raw output, and independent review for its ten gate rows. Those scoped requirements are not generalized to every package evidence requirement. An `acceptance_class` token cannot create a ticket, lease, qualification, accepted handoff, gate receipt, or launch-state advance.

Remaining contract work is explicit: close `OrderedAcceptedEvidenceRef.availability` and `OrderedUnavailableCheck.status`; bind requirements to producer commands or fixtures; define `artifact_required=false` representation against required output refs; and ensure accepted evidence resolves its acceptance class through the immutable ticket. These gaps are not defined here.

## Source anchors and digests

All digests below are SHA-256 of the source files at reviewed base commit `24ab0876ecff1bbf990a5ed140ee72403e865187`.

| Source | Relevant anchors | SHA-256 |
|---|---|---|
| `swarm/schemas/types-v1.toml` | 117–151 `ClosedReasonCode`; 176–181 `ClosedEnum`; 298–318 fixture/command outcome; 378–412 command/evidence/unavailable requirement; 426–480 reused output records | `BF27C94C86224E64AA59809FCB0117A3913F66B86A5582B7BFADBB704C7828C7` |
| `swarm/schemas/assignment-ticket-v1.toml` | 163–189 ordered fixture, command, evidence, and unavailable fields | `5DACF76CAA74DFF1766393267AC9F44E5455439F8E760999B9184B9BE9DCE339` |
| `swarm/ticket-drafts/p00/search-contracts.toml` | 1–6 nonclaimable draft state; 56–80 required outputs and named evidence | `4B345160CA1471FB8C5E00C95B42994D67DBE9D434C855EE0318555C959166EE` |
| `qualification/ticket-issuance/baseline.toml` | 127–133 result values and qualification-specific PASS rules | `4660AEDD1808DBE1A8E2099A931981FF5D9451C96D805F082B70EFDD7C0D3240` |
| `qualification/ticket-issuance/probes.toml` | 10–12 result/expected classes; 652–655 unavailable evidence; 912–918 invariants | `76BEDD3653905916BE0B87968CA25A19B5E22FFE8F413D84EBAB9FD75C45B3C1` |
| `qualification/ticket-issuance/TICKET_ISSUANCE_QUALIFICATION.md` | 124–151 evidence model and qualification-specific acceptance | `4176B919D1AA56047E6C3412EF74202708430B00F2698BA8984CF463726F4C10` |
| `swarm/p00-foundation-acceptance.toml` | G0 evidence rows: `required_state=PASS`, `current_state=UNAVAILABLE`, raw output and review required | `2D2C06DC8C747B646C053F5127BEE4EDD9090AE4402C38321610428E47FF3F8C` |
| `docs/handoff/P00_FOUNDATION_ACCEPTANCE_MATRIX.md` | 130–138 submission evidence; 159–194 G0-specific acceptance | `EDBF0CA4FD2BDA1C0BA2E409D92BF4ED29D5F1AAFC06941A61EC3E58EC37AA0C` |
| `docs/handoff/TICKET_ISSUANCE_OPERATIONS.md` | 320–353 cancellation/unknown outcomes; 355–392 closed failure registry | `AE93D8165A56FA1773077E9471CF7D7446FB8A9D099FF0D5C5281760F9556B46` |

The scoped codebase-memory graph query returned `xtask/src/accepted_evidence.rs:72` as a consumer anchor for `evidence_class`; its current validator checks generic uppercase syntax at line 146 and does not establish this proposal's exact allowlists. The proposal therefore remains a schema/contract candidate, not a claim about enforced runtime behavior.
