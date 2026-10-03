# Writer acknowledgement v2 design candidate

**Status:** PROPOSED_REVIEW_PENDING; **claimability:** NONCLAIMABLE. This proposal creates no record, identity, lease, ticket, signature, profile, operation receipt, qualification, or authority. It does not authorize implementation or issuance.

## Decision proposed

Close the existing acknowledgement request with one registered value type, WriterAcknowledgementV1, embedded only in event.writer_acknowledgement of a new lease_event_v2. Keep lease_event_v1 readable as read-only compatibility. A lease that is acknowledged starts its v2 chain with ACKNOWLEDGED; the pinned LEASED-to-SUPERSEDED path is also preserved as a pre-ACK SUPERSEDED genesis. Existing v1 chains cannot be extended, mixed with v2, or treated as v2 predecessors. This proposal adds no pre-ACK REVOKED genesis.

The field paths, types, canonical order, projection digests, v2 event descriptor, instance profile, and operation revisions are spelled out in the sibling TOMLs under swarm/schema-drafts/lease-event-v2/. They are review candidates; none is an actual registry or schema.

## Exact acknowledgement shape

WriterAcknowledgementV1 has the following ordered fields:

1. writer_identity: ActorIdentity
2. lease_ref: ImmutableRecordRef of writer_lease_v1
3. assignment_ticket_ref: ImmutableRecordRef of assignment_ticket_v1
4. context_manifest_ref: ImmutableRecordRef of context_manifest_v1
5. context_artifact_sha256: Sha256Digest
6. base_commit: GitObjectId
7. worktree_ref: OpaqueWorktreeRef
8. write_scope_digest: Sha256Digest
9. dependency_handoffs_digest: Sha256Digest
10. required_commands_digest: Sha256Digest
11. required_evidence_digest: Sha256Digest
12. unavailable_checks_digest: Sha256Digest
13. line_limits_digest: Sha256Digest
14. ticket_type_registry_sha256: Sha256Digest
15. statement: ClosedEnum, with the sole value ACKNOWLEDGED_PREIMPLEMENTATION.

Every immutable reference contains its exact complete-file SHA-256. The payload binds the writer, lease, ticket, context, artifact, base, worktree, scope, dependency handoffs, command/evidence/unavailable obligations, line limits, and the exact type-registry file digest selected at the ticket base commit. Cross-record equality is checked against the exact ticket, lease, context, and event actor. It contains no free text, source bytes, secret, execution result, qualification claim, or PASS assertion.

event.writer_acknowledgement_digest is outside the payload and required exactly when event.kind = ACKNOWLEDGED. The payload and digest are both forbidden for SUBMITTED, REVOKED, and SUPERSEDED. There is no digest field inside the payload and no separate acknowledgement record or artifact.

## Canonical digests and ordering

The payload digest is SHA-256(ASCII("ELIOT-SEARCH/WRITER-ACKNOWLEDGEMENT-PAYLOAD/V1") || 0x00 || canonical_payload_bytes).

canonical_payload_bytes are the 15 declared fields rendered as ordered UTF-8 TOML assignments, with LF endings and one final LF. There is no BOM, comment, trailing whitespace, null, or implicit table ordering. Existing scalar types in this projection are ASCII. ImmutableRecordRef values use its registered field order: repository, commit, path, git_blob_id, exact_record_file_sha256, record_kind. OptionalV1 values retain both state and value; absence is never null or omission.

The six obligation digests each use their own ASCII domain string, one NUL byte, and the exact canonical projection document:

SHA-256(ASCII(domain) || 0x00 || projection_document_utf8_lf_bytes)

The domains and projection inputs are:

| Payload field | Domain | Exact source and canonical order |
| --- | --- | --- |
| write_scope_digest | ELIOT-SEARCH/ACK-WRITE-SCOPE/V1 | repository_fence.write_scope |
| dependency_handoffs_digest | ELIOT-SEARCH/ACK-DEPENDENCY-HANDOFFS/V1 | dependencies[], each OrderedAcceptedPackageHandoff in its registered field order; source order is retained and the ticket schema requires sort by package |
| required_commands_digest | ELIOT-SEARCH/ACK-REQUIRED-COMMANDS/V1 | evidence.required_commands[], each BoundedCommandSpec in its registered field order and ticket order |
| required_evidence_digest | ELIOT-SEARCH/ACK-REQUIRED-EVIDENCE/V1 | evidence.required_evidence[], each EvidenceRequirement in its registered field order and ticket order |
| unavailable_checks_digest | ELIOT-SEARCH/ACK-UNAVAILABLE-CHECKS/V1 | evidence.unavailable_checks[], each UnavailableCheckRequirement in its registered field order and ticket order |
| line_limits_digest | ELIOT-SEARCH/ACK-LINE-LIMITS/V1 | limits.soft_src_lines, limits.split_review_total_lines, limits.hard_total_lines, in that order; static_context_artifacts is excluded |

Projection documents use the exact root field order recorded in writer-acknowledgement-v1.toml. Ordered records are compact inline TOML tables using the registered canonical_fields; ordered arrays preserve the exact issued-ticket order and use [] when empty. Scalars use canonical TOML values; optional values use the registered OptionalV1 state/value pair. No sorting, normalization, omission, implicit null, or reserialization from a different record is allowed.

The accepted ticket-enum decision at `bac90ff7753ef7d4d107836c60480df216acfe7f` adopts exactly four named domains and nine field bindings. The writer validates these exact values in the `types-v1.toml` file selected by `swarm/control-plane-schema.toml` at the ticket base commit:

| Named type | Exact allowed values | Bound fields |
| --- | --- | --- |
| `FixtureQualificationStatus` | `FAILED`, `QUALIFIED`, `UNAVAILABLE` | `OrderedFixtureRef.qualification_status` |
| `NormalProcessExitClass` | `EXIT_NONZERO`, `EXIT_ZERO` | `BoundedCommandSpec.expected_exit_class`; `OrderedRawCommandOutcomeRef.exit_class` |
| `EvidenceArtifactClass` | `PACKAGE_HANDOFF_CANDIDATE`, `PUBLIC_API_SCHEMA_DIGEST`, `QUALIFICATION_PROBE_RESULT`, `RESIDUAL_RISK_RECORD`, `TEST_RESULT` | `BoundedCommandSpec.evidence_class`; `EvidenceRequirement.evidence_class`; `OrderedEvidenceRef.evidence_class`; `OrderedAcceptedEvidenceRef.evidence_class` |
| `ExpectedBehaviorClass` | `FAILURE`, `POLICY`, `RECOVERY`, `SUCCESS` | `EvidenceRequirement.acceptance_class`; `OrderedEvidenceRef.acceptance_class` |

The writer checks each occurrence at those nine paths against its named type's exact allowlist before hashing the issued ticket obligations; values are canonical case-sensitive strings, and unknown values or aliases are rejected. These values describe classifications, not successful execution or evidence acceptance: `QUALIFIED` still requires exact-fixture immutable evidence and independent review; an expected `SUCCESS` is not a `PASS`; and `EvidenceArtifactClass` does not accept evidence. Preserve an observed `EXIT_ZERO` or `EXIT_NONZERO` even when it mismatches the expected class. Unknown normal termination remains unavailable, not `EXIT_NONZERO`. The accepted decision adds no producer-command link, unavailable-check command link, exactly-once outcome coverage, or array-order association.

Field 14, `ticket_type_registry_sha256`, is the SHA-256 of the exact committed Git-blob bytes of the selected `swarm/schemas/types-v1.toml`. The proposal targets control-plane schema version 5, type-registry schema version 2, and record-schema version 1. In this e58 snapshot, the accepted enum decision advances the historical 47-type baseline to 51 and the proposed `WriterAcknowledgementV1` adds one, for a historical proposal target of 52. The separately accepted `QualifiedOpaqueId` grammar adoption occurred after e58 and is outside this proposal's pinned source set; if both adoptions are materialized, the future cumulative count is 53 (51 + 1 + 1). Neither number claims a current or implemented registry. The canonical registry implementation remains pending, so its final file digest is unknown and is not fabricated here. The writer must resolve the registry path from the exact control-plane schema at `base_commit`, validate the four accepted enum entries and nine bindings plus every other type adoption accepted at that base, and place the actual complete-file digest in the payload. Since this is a canonical payload field, changing the selected registry bytes changes the ACK payload digest and writer event signature. The 47-type baseline digest is not a valid ACK value. No digest cycle exists: the field names the immutable registry file at the ticket base commit, not a value embedded in that file. Generic `ClosedEnum` fields outside adopted bindings remain outside this decision.

The accepted decision does not adopt producer-command links, unavailable-check command links, exactly-once coverage across command outcome arrays, or any association inferred from array order. This ACK proposal adds none of those requirements.

Operation IDs have a separate role and formula: the proposed v2 operation ID is SHA-256(ASCII(domain_separator) || canonical_operation_input_bytes), without a NUL delimiter, as the current ticket-issuance contract specifies. The proposed operation IDs, input schema IDs, and domains are listed in operations-v2.toml. This operation-input byte encoding is not defined here by importing the unaccepted c209 TLV draft.

## Event v2 and lifecycle rules

The proposed lease_event_v2 schema keeps the v1 root field order, with canonical_field_order at the TOML root before [event_reason_codes], as required by the accepted descriptor correction. It adds the two conditional ACK fields inside event; their nested order is payload then payload digest. The event signature still covers the exact UTF-8/LF record bytes before [signature]. The embedded signature.record_sha256 is the signed-payload digest; the complete-file digest remains external.

The ACK event has exactly two related_records[] values in this order: the exact assignment ticket, then the exact context manifest. These must equal the payload refs; the lease stays in lease.ref. The assigned writer signs the exact event preimage. The integration owner may verify and record those unchanged bytes but cannot sign as the writer.

The existing event reason mapping and actor rules remain closed:

| Kind | Reason | Actor | Related-record meaning | Lifecycle effect |
| --- | --- | --- | --- | --- |
| ACKNOWLEDGED | WRITER_ACKNOWLEDGED | exact assigned writer | ticket, then context manifest | LEASED to IMPLEMENTING only after exact readback |
| SUBMITTED | PACKAGE_SUBMITTED | exact assigned writer | exact package_submission_v1 emitted in the same commit | IMPLEMENTING to REVIEW; submission is not acceptance |
| REVOKED | LEASE_REVOKED | integration owner | existing v1 rule names a revocation receipt | terminal authority removal |
| SUPERSEDED | LEASE_SUPERSEDED | integration owner | exact replacement record named by the matching supersession receipt | terminal supersession; old record bytes remain unchanged |

An ACKNOWLEDGED genesis is allowed only for the exact assigned writer while the source lease state is LEASED, with no earlier lease event. The pinned orchestration contract also permits LEASED to SUPERSEDED: for this pre-ACK path, integration owner records SUPERSEDED as the first v2 event, binds the exact replacement named by a supersession receipt whose old-record ref and complete-file digest equal the lease ref, and leaves both previous-event optionals ABSENT. Old lease bytes remain unchanged. These are the allowed genesis patterns in this proposal. It does not add an empty-chain REVOKED transition: the pinned orchestration transitions do not establish LEASED to REVOKED, and a generic terminal-operation input is not authority to invent that path. Every non-genesis event points to the immediate predecessor for the same lease with its exact complete-file digest; gaps, forks, cross-lease links, duplicate predecessors, and mixed versions reject. lease_event_v1 stays read-only. REVOKED and SUPERSEDED are terminal and permit no later event. SUBMITTED requires the prior ACK and enters review; it does not itself expire or accept the lease/package.

The accepted instance profile value is RECORDED, in the proposed lease_event_instance_v2. It identifies the serialized event instance only; by itself, it does not prove operation success or exact readback. It never means ACKNOWLEDGED, current authority, implementation permission, evidence success, package acceptance, a handoff, a gate, or wave advancement.

## Operation revisions and profile boundary

The proposed operation registry schema version is 2. Each operation definition separately names operation_schema_version = 2, input_schema_version = 2, its input_schema_id, and its distinct domain_separator. These identifiers have separate roles: the operation ID names the mutation, the input schema ID/version closes the typed input shape, and the domain string separates the operation hash. All three lease-event producers have v2 values:

- acknowledge_writer_lease_v2 / acknowledge_writer_lease_input_v2 / acknowledge_writer_lease_v2
- record_package_submission_v2 / record_package_submission_input_v2 / record_package_submission_v2
- revoke_or_supersede_lease_v2 / revoke_or_supersede_lease_input_v2 / revoke_or_supersede_lease_v2

The ACK operation takes exact lease, ticket, context refs, context-artifact digest, typed acknowledgement payload, and its digest; it requires a LEASED source state with no prior v1/v2 event and emits the ACKNOWLEDGED genesis. It returns only lease_event_v2_ACKNOWLEDGED. The submission operation retains the package_submission_v1 record and atomically emits lease_event_v2_SUBMITTED after the one valid ACK. The terminal operation consumes a validated linear v2 chain; an empty chain is permitted only for the pinned LEASED-to-SUPERSEDED pre-ACK path, which emits a SUPERSEDED genesis with both previous-event optionals ABSENT. Empty-chain REVOKED is not added. V1 operation IDs remain distinct and read-only for compatibility; v1 and v2 are never aliased.

The c209 native issuance-profile worktree is explicitly an unaccepted candidate. Its operation-input draft covers acknowledge_writer_lease_v1, says a schema/domain change requires a new exact v2 binding, and includes a separate acknowledgement artifact input. The embedded payload proposed here has no such artifact. The c209 draft’s TLV bytes, receipt schema, approval/store profiles, trust records, and any keys or signatures are not imported as normative. A reviewed v2 operation-input/profile revision must bind the IDs, input schemas, exact input bytes, signature roles, and durable recovery rules before any operation can run. No operation is enabled in these draft TOMLs.

## Revisions, bounds, and impact

The historical pinned baseline is control-plane schema v4, type registry format v2, operation registry v1; nine required schema files; eight closed record kinds; 47 registered types; four required instance profiles. The accepted enum decision independently takes the historical type count from 47 to 51 while retaining type-registry format 2 and record-schema version 1. This e58 proposal snapshot targets control-plane schema v5, type registry format v2, operation registry v2; ten schema files; nine record kinds; 52 types; five instance profiles. Relative to the accepted 51-type enum scope, this ACK proposal adds one value type, `WriterAcknowledgementV1`; 52 is the historical e58 target only, not a current registry count. The separately accepted `QualifiedOpaqueId` grammar adoption postdates e58 and is excluded from the source snapshot; if both adoptions are materialized, the future cumulative target is 53, with implementation and final registry digest still unknown. The delta also adds one `lease_event_v2` schema/kind and one v2 instance profile. No standalone writer-acknowledgement record kind is added. The separate operation-receipt/trust design is out of scope and has no count delta here.

The proposed canonical payload cap is 16 KiB and complete event cap 32 KiB. Current type maxima for three record refs, actor/worktree IDs, and eight digest strings place the ACK payload below 5 KiB. Those bounds are proposals for review, not global issuance limits. The ticket schema bounds dependencies at 32, required commands at 64, and unavailable checks at 64, but does not declare a required_evidence[] cardinality or a finite ticket-file byte cap. Streaming its digest does not close the bounded-operation requirement; no cap is invented here, and the v2 operation remains disabled until that inherited contract is fixed.

## Unresolved blockers

1. The four named enum domains and nine bindings are accepted, but canonical `types-v1.toml` implementation remains pending. The ACK field and validator rule are proposed; the final registry file digest is unknown until that implementation is reviewed and accepted.
2. lease-event-v1.toml requires REVOKED_binds_revocation_receipt, while the current ClosedControlRecordKind allowed set has no revocation-receipt value. A separately reviewed contract change must define the type/reference binding; v2 issuance remains disabled until then.
3. assignment-ticket-v1 lacks a maximum required_evidence[] count and ticket-record byte cap. Bounded issuance cannot be claimed.
4. The c209 operation-input/receipt profile is unaccepted, currently has only the v1 acknowledgement input, and is under separate review. It cannot provide the v2 mutation, canonical input-byte, durable recovery, or profile binding as-is.
5. No assigned writer identity, integration-owner signing identity, approved signer roles/profile, key, signature artifact, artifact store, trust state, actor qualification, authoritative Git write, event, or readback is present.
6. The later accepted QualifiedOpaqueId grammar adoption is not in the e58 source pins; canonical registry implementation must reconcile it with WriterAcknowledgementV1, yielding a future cumulative target of 53 if both are materialized. The exact final registry file digest remains unknown.

The independent reviewer must decide the proposed revision numbers, digest encodings, exact type-registry digest binding, exact v2 operation input binding, and the two inherited closure blockers. Until that review and registry acceptance, every draft remains non-claimable.

## Exact source pins

The machine-readable source table is in `swarm/schema-drafts/lease-event-v2/manifest.toml`. Every row names a full immutable commit ID, exact path, Git blob ID, and SHA-256 over the raw decompressed Git blob payload bytes returned by `git cat-file blob`; no checkout line-ending conversion or working-tree bytes are hashed. The acknowledgement request is pinned to commit 859aec8737f441291b383ca6ff71c33de24b4ce8 (SHA-256 b251f8e5eae6b8ce39af113ba8730cc5f7ddeb7942798a218805e7b81e91d45f). The four-domain/nine-binding enum decision is pinned to accepted main commit bac90ff7753ef7d4d107836c60480df216acfe7f. Its 47-type registry snapshot is only the pre-implementation baseline, not the future ACK digest. The accepted instance/status decision and v4 historical registry baseline are pinned at cac9c437ae928051b6e8a0824c11718d50a573f3. The native issuance-profile comparison is pinned at c209addae7dc379191ce14dc2f0b72f176e79cfa and explicitly labeled unaccepted. The later QualifiedOpaqueId grammar decision is outside the e58 source snapshot and is noted only as a cumulative-scope caveat; this candidate does not claim its registry implementation or digest.

The codebase-memory graph is not used as authority for these contract facts; exact source files are hash-pinned above. No ELIOT diagnostic, runtime, issuance, signing, qualification, or external service result is claimed.
