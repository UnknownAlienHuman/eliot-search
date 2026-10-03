# Contract change request

## Identity

- Requesting package: integration-owner control-plane tooling
- Contract-owning package: search-contracts for shared types/records; integration-owner for the operation registry
- Base commit: `99db410fc540104928284f2ecd206b4b5baec8a0`
- Architecture sections, only when needed: None. This request concerns the missing control-plane acknowledgement encoding; it does not request an Architecture Part I change.
- Status: `PROPOSED_REVIEW_PENDING`. This document is a proposal only. It accepts no contract change, supplies no signature or profile, creates no ticket/lease/event, and grants no implementation authority.

## Blocking problem

The current normative contract already requires all of the following:

- `docs/handoff/TICKET_ISSUANCE_OPERATIONS.md` §1 and §8 requires an exact assigned writer to acknowledge the ticket, lease, context, base commit, worktree, write scope, dependency handoffs, command/evidence obligations and line limits. Success is an append-only `lease_event_v1` with `event.kind = ACKNOWLEDGED` and reason `WRITER_ACKNOWLEDGED`; implementation starts only after exact readback.
- `swarm/control-plane-operations.toml` operation `acknowledge_writer_lease` takes `writer_acknowledgement` and emits `lease_event_v1_ACKNOWLEDGED`.
- `swarm/schemas/lease-event-v1.toml` requires the acknowledgement event's actor to be the ticket writer and its signature to bind that actor. It also requires `related_records[]` and says `ACKNOWLEDGED_binds_ack_digest`.
- `swarm/WRITER_LEASE_TEMPLATE.md` says the acknowledgement is separate from the lease and is represented by the append-only acknowledgement event.

The load-bearing representation is missing. `writer_acknowledgement` has no registered type or record schema. The event schema has no acknowledgement payload or digest field. Its `related_records[]` elements are `ImmutableRecordRef`, whose `record_kind` must be one of the eight values in `swarm/schemas/types-v1.toml` `ClosedControlRecordKind`; there is no writer-acknowledgement record kind. A validator therefore cannot determine which bytes `acknowledgement` means, what digest the event must bind, or how to validate the writer's claimed checks. Treating an unregistered kind, arbitrary digest, or untyped signature artifact as the acknowledgement would be a local workaround, not a schema-valid issuance.

The selected approval/signature profile and actual writer signature remain external authority inputs. `ImmutableSignatureRef` deliberately does not choose a cryptographic algorithm without a profile. This request defines neither identities nor signing authority.

## Producer and consumers

- Producer: the exact assigned writer approves the canonical v2 event preimage through a selected approval profile. The integration owner validates the writer-bound signature and records the acknowledgement event under `acknowledge_writer_lease`.
- Current consumers: the lease-event schema validator, operation engine, event-chain recovery, and orchestration state transition from `LEASED` to `IMPLEMENTING`.
- Future compatibility surface: any native Rust control-plane implementation, event-chain reader, ticket/lease auditor, and later submission/review operations that require an acknowledged active lease.

## Proposed contract

The recommendation is to make the writer acknowledgement a typed payload **inside a new versioned append-only `ACKNOWLEDGED` lease event, `lease_event_v2`**. This matches the existing template's event-as-acknowledgement shape, preserves the closed `related_records[]` type, and avoids a second immutable record write. Keep `lease_event_v1` readable but read-only; new leases use v2 for every lifecycle event so a lease chain does not mix event schemas. The new event remains the only acknowledgement state transition and is signed by the exact assigned writer, as the current schema already requires.

Define a closed `WriterAcknowledgementV1` payload, required only for `event.kind = ACKNOWLEDGED` in `lease_event_v2` and forbidden for other event kinds. The operation input `writer_acknowledgement` is this exact payload. It contains no free text, source bytes, secrets, or execution results. Its fixed fields bind:

1. the exact writer `ActorIdentity`;
2. the exact immutable lease, assignment-ticket and context-manifest references, including their complete-file digests, plus the context artifact SHA-256;
3. the ticket base commit and opaque worktree reference;
4. canonical digests of the ticket/lease write scope, dependency-handoff set, required command set, required evidence set, unavailable-check set and line-limit tuple; and
5. one closed statement such as `ACKNOWLEDGED_PREIMPLEMENTATION`, meaning the writer accepts these exact obligations before implementation. It must not imply that a command ran, evidence exists, or a check passed.

The acknowledgement digest is `SHA-256("writer_acknowledgement_payload_v1" || canonical_payload_bytes)`, where the canonical payload bytes have a declared field order and exclude the digest field itself. Use the existing UTF-8/LF and ordered-field rules. The v2 event's signed-payload digest/signature then binds the acknowledgement table and its digest. Require all identity and reference equalities: acknowledgement writer = event actor = ticket writer = lease writer; acknowledgement lease/ticket/context refs and digests = the operation inputs; base, worktree, scope and dependency digests = the issued ticket/lease; command, evidence, unavailable-check and line-limit digests = the issued ticket. The acknowledgement operation uses a versioned input/domain identity (`acknowledge_writer_lease_v2`) and returns `lease_event_v2_ACKNOWLEDGED`.

The operation first fixes its canonical input manifest and operation ID, then fixes the event ID, recorded timestamp and all unsigned event fields. The assigned writer signs the resulting exact UTF-8/LF pre-signature bytes; the event's existing `ImmutableSignatureRef` points to the actual signature artifact and selected approval profile. The signature artifact is an operation result, not part of the signed preimage or a self-referential operation-ID input. The integration owner verifies it, commits the event unchanged and performs exact readback. Retries reuse the durable operation result/signature reference; no integration-owned signer can act as the writer.

For an `ACKNOWLEDGED` v2 event, define `related_records[]` to contain exactly two existing control-record refs, in the declared order: the assignment ticket and context manifest. The lease remains in `lease.ref`. These refs must equal the refs inside the acknowledgement payload. `related_records[]` does **not** point to an acknowledgement record or artifact. Replace the currently underspecified rule with the equivalent of `ACKNOWLEDGED_binds_embedded_ack_digest_and_exact_ticket_context_refs`. This makes the digest binding explicit without adding an unregistered `ImmutableRecordRef` kind.

Version the event shape explicitly: add `lease_event_v2` and its schema, retain `lease_event_v1` for read-only compatibility, and require all event producers for a new lease to append v2 events. A v2 chain starts without a v1 predecessor; a mixed-version chain is rejected. The operation registry, schema registry and event-chain reader must agree on the v2 output and domain separator. This is preferable to silently adding required fields to a strict `unknown_fields = "reject"` v1 record.

Bounds and state rules:

- exactly one acknowledgement payload and one `ACKNOWLEDGED` transition per lease;
- exactly two related-record refs for that event;
- no arrays, free-form text, source content or secrets in the acknowledgement payload; its size is fixed apart from already typed identifiers and refs (proposed cap: 16 KiB canonical payload and 32 KiB complete event);
- the payload digests refer to the existing bounded ticket/lease sets and do not copy their contents;
- identical operation ID and canonical input returns the original event; the same operation ID with different bytes fails `CONTROL_OPERATION_CONFLICT`; a different acknowledgement after the lease has left `LEASED` fails closed using the existing acknowledgement mismatch/state failure; and
- after a possible write, recovery must read back the operation receipt and exact event bytes, recompute Git blob/file digests and signature binding, and verify the event chain. If absence cannot be proved, outcome remains unknown; no blind retry or second event is allowed.

Identity and evidence binding:

- The writer ID is the exact ticket/lease `ActorIdentity`, not a display name. The event signature reference must bind the same actor and the exact signed-payload digest.
- The payload binds the obligations by digest only. It acknowledges the assigned base/worktree/scope and evidence plan; it does not certify test execution, `PASS`, package acceptance, a handoff or a gate.
- The integration operation still needs a configured approval profile, an actual writer signature artifact, and authoritative verification/readback. A structurally valid payload or local candidate remains advisory until those checks and the exact Git event readback succeed.

Alternative considered: persist a separate `writer_acknowledgement_v1` control record at `swarm/leases/<package>/acknowledgements/<ack_id>.toml`, with the same fixed references/digests and closed preimplementation statement, signed by the exact writer; make the v2 event's `related_records[]` contain exactly the ticket, context and acknowledgement refs. This is explicit and independently addressable, but adds a second signed/read-back record and a second immutable write whose recovery must be coordinated with the event. Because the current event actor/signature rule also requires the exact writer, the event still needs its own writer-bound signature; it cannot reuse the acknowledgement-record signature over different bytes. The embedded form is smaller and keeps one signed writer event as the state transition. The separate-record option would add one more record kind/schema/file on top of v2 (ten record schemas, eleven required schema files, ten closed record kinds); no unregistered acknowledgement kind may be assumed.

Implementation boundary: after contract acceptance, the smallest standalone native Rust slice is the closed `WriterAcknowledgementV1` type, canonical encoder/digest, and cross-record validator against exact ticket/lease/context instances, followed by deterministic event fixtures. This is structural conformance only. The mutation slice must then verify the exact writer signature, append the v2 event and operation receipt, and perform complete Git/event-chain readback. These operations can live in native Rust integration tooling (for example, `xtask`) and run locally without ELIOT Governor or Memory OS. Standalone operation does not waive the documented authority inputs: the writer identity/signature profile and artifact, integration control-store/Git write authority, and exact post-write readback must be configured and verified. Materializing contexts additionally still requires a selected artifact-store profile and its durable readback. No default profile or implicit local-store authority is proposed here.

## Impact

- Security/access: binds the acknowledgement to the exact assigned writer and immutable ticket/lease/context; prevents arbitrary related-record kinds and free-form assertions. Signature verification still depends on a real, selected approval profile.
- Currentness/epoch/publication: permits only the existing `LEASED` to `IMPLEMENTING` transition after exact event readback. It does not publish search data, accept a package, advance launch state, or create a handoff.
- Source identity/readback: uses the existing immutable Git refs and complete-file digests; the event signature covers the embedded acknowledgement digest and payload. No source bytes are copied.
- Residency/retention/purge: the event remains append-only under the existing lease-event path. The acknowledgement adds no source residency or purge behavior; signature artifact retention remains governed by the selected profile.
- Protocol/wire compatibility: retain v1 parsing as read-only and issue v2 event records only. The recommended embedded form adds one closed record kind (`lease_event_v2`), one record schema/file, one required schema file and one registered value type: current counts become 9 record schemas, 10 required schema files and 48 registered types. The separate-record alternative adds another kind/schema/file (10 record schemas, 11 required schema files, ten closed kinds); its standalone record can use existing field types. Bump the control-plane/type-registry schema revisions consistently with the accepted v2 contract; the exact revision numbers must be set in the registry change, not inferred by this proposal.
- Migration/backward compatibility: the base control-plane disposition says implementation is `ABSENT` and issued records are `0`, so there are no issued event chains to rewrite. Strict validators still need a coordinated v2 update before issuance. V1 event records, if ever encountered, remain read-only and cannot be extended by a v2 event under this proposal.
- Resource budget: one bounded payload and two refs per acknowledgement; proposed caps are 16 KiB for canonical acknowledgement payload and 32 KiB for the complete event. The acknowledgement stores hashes of the existing plan sets, not their elements. Review should confirm or adjust these caps against the existing canonical type-size limits before accepting the change.

## Tests

- Deterministic canonical payload encoding and golden digest; verify the acknowledgement digest excludes only its own digest field and is domain-separated from the operation ID.
- Valid payload/event fixture proves the writer, lease, ticket, context, base, worktree, scope, dependencies, commands, evidence requirements, unavailable checks and line limits all match the immutable inputs.
- Negative fixtures reject wrong/non-ticket writer, wrong signature actor/profile binding, any mismatched ref or digest, a third/unknown related-record kind, missing/extra related refs, acknowledgement on non-ACK events, missing acknowledgement on ACK events, unknown fields, free text, source bytes, secrets, and attempted `PASS`/execution claims.
- Property tests mutate each bound digest/reference independently and prove validation fails; canonical field/list order changes alter exact identity.
- Fault tests cover cancellation before write, cancellation after possible commit, exact event/operation-receipt readback, missing or conflicting event-chain links, same-input idempotent replay, same-operation/different-input conflict, and a second acknowledgement after the lease leaves `LEASED`.
- Conformance fixtures prove v1 is read-only, v2 accepts the closed acknowledgement only on `ACKNOWLEDGED`, v2 event chains never mix with v1, and old strict readers reject v2 rather than silently accepting new fields.

## Decision needed

Before normative acceptance, the integration owner and independent reviewer must fix all of these
load-bearing details in the versioned schema change:

- the exact `WriterAcknowledgementV1` field paths, registered types and canonical field order;
- the exact byte encoding, order and domain separator for each obligation-set digest and the line-limit
  tuple, rather than accepting an unspecified "canonical digest";
- the operation registry identifier, input schema revision and domain separator, with their distinct
  roles explicitly stated;
- the v2 rules for `SUBMITTED`, `REVOKED` and `SUPERSEDED`, preserving the existing actor, reason,
  related-record, event-chain and terminal-state constraints in every new v2 lease chain; and
- the exact v2 instance-status binding coordinated with the sibling instance-status change. The
  proposed `RECORDED` status identifies a serialized event instance; it does not imply acknowledgement,
  current lease authority, execution evidence or acceptance.

This checklist remains unresolved. Publication of this request does not close these decisions or
authorize a reader, writer or mutation engine to infer them.

Classify as one of:

- clarification inside the current contract;
- compatible contract extension;
- **breaking contract revision**;
- ADR-required implementation default;
- Architecture 8.4 revision required.

Recommended classification: **breaking contract revision**, because the current event schema rejects unknown fields and leaves the acknowledgement digest undefined. No existing instances need migration, but schema readers must be versioned or updated explicitly. Review must decide between the recommended embedded v2 payload and the separately persisted acknowledgement record, and confirm the operation/schema revision identifiers. The proposal recommends the embedded v2 payload on auditability-to-complexity grounds; no ticket, lease, event, signature or profile is authorized by this request.
