# Contract change request — control-record instance closure

**Status:** PARTIALLY_ACCEPTED_INTEGRATION_CONTRACT (2026-10-03). The four instance-status values and exact registry bindings are accepted below. The five ticket enum fields and their reused output vocabularies are accepted separately in `2026-10-03-ticket-enum-adoption.md`; bounded binding/value validation is implemented at `82315138f337ecbcab5ccb1a3f01b67330930572`, while full canonical instance validation remains unresolved. No decision creates an issued record.

## Identity

- Requesting package: integration-owner tooling.
- Contract-owning package: integration-owner control plane.
- Base commit: `99db410fc540104928284f2ecd206b4b5baec8a0`.
- Architecture sections: none required; the missing fields are integration metadata.

## Blocking problem

The independent Luna audit identified two load-bearing gaps in the first four record schemas.

1. Each canonical field order includes `status`. Only `context_manifest_v1` has a linked instance profile: `swarm/context-manifest-instance-v1.toml` declares the exact status `MATERIALIZED`. Assignment-ticket, writer-lease, and lease-event schemas instead declare the descriptor status `SCHEMA_ONLY_NOT_AN_INSTANCE` and provide no instance status field/profile. Orchestration states and draft statuses do not define a serialized instance mapping.
2. `ClosedEnum` requires a field-specific exact allowed set or equality rule. Several nested ticket fields only say `is_ClosedEnum`: `OrderedFixtureRef.qualification_status`, `BoundedCommandSpec.expected_exit_class`, `BoundedCommandSpec.evidence_class`, `EvidenceRequirement.evidence_class`, and `EvidenceRequirement.acceptance_class`. No exact field binding or allowed set is provided in the type registry.

An implementation cannot infer status strings from workflow prose or accept every uppercase string as a closed enum. The known context-manifest format/materialization and writer-lease lifecycle equality rules remain usable and must be preserved.

## Producer and consumers

- Producer: integration owner defines and reviews the instance and enum profiles.
- Current consumers: schema descriptors and advisory planners; no authoritative records exist at the base.
- Future consumers: closed-schema instance validation, canonical rendering, issuance, exact readback, and lease acknowledgement.
- Compatibility surface: exact versioned profiles bound by immutable operation inputs and schema-registry digests.

## Proposed contract

### Record instance statuses

Add an explicit versioned instance profile for each of the first four record kinds and bind those profiles in the general control-plane schema registry, rather than only in an advisory planner.

The recommended new values are:

| Record kind | Proposed sole instance status | Existing or proposed |
| --- | --- | --- |
| `context_manifest_v1` | `MATERIALIZED` | Existing instance-profile value |
| `assignment_ticket_v1` | `ISSUED` | Proposed integration choice |
| `writer_lease_v1` | `LEASED` | Proposed integration choice |
| `lease_event_v1` | `RECORDED` | Proposed integration choice |

These values identify immutable record instances. They do not replace orchestration state, prove currentness, create a lease, or advance a wave. In particular, `RECORDED` does not mean `ACKNOWLEDGED`; the event kind/reason and exact actor/lease binding still decide that operation's meaning. Reject unknown values and the descriptor placeholder status.

### Nested enum bindings

Give every nested `ClosedEnum` field a versioned, machine-readable, exact field-path binding. The binding must name one finite allowed set or one equality value; no free-form uppercase fallback is permitted. Alias and array-element resolution must preserve the exact originating field path.

For the five unresolved ticket fields, the integration decision must define:

- fixture qualification states and the immutable qualification evidence needed to assert each state;
- expected command exit classes and their mapping to actual raw process outcomes;
- evidence classes, with a registry that covers the ticket's exact required checks;
- acceptance classes, including which outcomes are required and how unavailable results remain visible without satisfying a mandatory requirement.

This request deliberately does not invent those class names or equate fixture qualification with a probe's PASS/FAIL/UNAVAILABLE result. Their sets and semantics need a reviewed field profile. Existing stage, assignment, fixture-owner, and qualification registries remain the authorities for requirements; a profile cannot weaken them.

Runtime schema validation must reject an unresolved field binding even if the supplied value matches the generic `ClosedEnum` character grammar. Schema validation alone does not verify qualification, process outcomes, actor authority, or signature/store trust.

## Impact

- Security/access: prevents schema placeholders, workflow-state inference, and free-form classes from acquiring control authority.
- Currentness/epoch/publication: record status remains distinct from live orchestration state; publication/readback rules are unchanged.
- Source identity/readback: profiles must be loaded from exact immutable inputs and bound by digest.
- Residency/retention/purge: metadata only; no source bodies, local absolute paths, or secrets are added.
- Protocol/wire compatibility: integration record schemas only; Search provider wire types are unchanged.
- Migration/backward compatibility: no issued record needs migration at the base. Historical advisory/test captures remain identifiable and are not promoted into instances.
- Resource budget: profiles need finite entry/string limits; no global control-record cap is selected by this proposal.

## Tests

- Correct exact record-kind/status pairs accepted; descriptor, unknown, and cross-kind status rejected.
- Generic validator actually loads and binds the context instance profile from the schema registry.
- Each nested enum path resolves through its declared type aliases and array-element semantics.
- Missing, duplicated, wrong-owner, unknown, or conflicting enum profiles rejected.
- A string matching the uppercase grammar but absent from the allowed set rejected.
- Required unavailable evidence stays typed and visible; it does not become success.
- Canonical rendering/readback preserves the chosen status and ordered fields without reserializing accepted identity.

These are future acceptance cases, not executed qualification evidence.

## Decision needed

**Compatible control-plane contract extension**, subject to independent review and an explicit integration-owner decision on the proposed statuses and missing class profiles. Until that decision and its registry changes are accepted, validators may report the gaps but issuance cannot guess them.

## Partial integration decision — 2026-10-03

The integration owner accepts only the four exact kind/status pairs above and their versioned profile
bindings in `swarm/control-plane-schema.toml` version 4. Independent Luna review of source candidate
`231fa170ec33b385ea9591e2f689d520823083ee` found these values correctly separated from current authority
and orchestration state. The registry remains schema-only, with zero issued records and no accepted
handoffs. Serialized `status` must occur exactly once in each bound descriptor's canonical field order;
the audit found that this check still needs implementation before structural binding acceptance.

The integration owner also accepts the independently reviewed lease-event descriptor correction:
move its unchanged `canonical_field_order` array before `[event_reason_codes]`, so the array is root
metadata like the other descriptors. The original TOML table scope nested it inside the reason map;
the earlier PowerShell regex check ignored table scope and did not prove this layout correct. Preserve
schema version 1, every reason mapping, field, rule and order. This changes descriptor placement, not
instance wire fields or their canonical order. Typed profile binding must require the root array and
reject a nested-only fallback. No Architecture Part I change is needed.

Nested enums outside the separately accepted ticket subset, full-schema validation, canonical record rendering, actor/signature trust,
artifact/approval profiles, qualification and issuance remain unresolved and unaccepted by this partial
decision. A structural validator's `NON_AUTHORITATIVE` result satisfies none of those obligations.
The seven prior profile tests belong to the recorded candidate; they do not prove the subsequent fix.
