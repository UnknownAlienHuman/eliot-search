# Ticket-issuance plan digest rule

This supplement closes the advisory planner digest definition before implementation.

`plan_sha256` is **not** a self-hash. It is computed as:

```text
SHA-256(
  ASCII("eliot-search/ticket-issuance-plan/v1\\0")
  || canonical JSON bytes of the complete plan object with `plan_sha256` omitted
)
```

The emitted plan then appends `plan_sha256` as the final logical field. A complete-file SHA-256 may be
recorded externally by a caller, but it is a different value and is not embedded in the plan.

Implementations and validators must reject:

- hashing a placeholder value and replacing it afterward;
- fixed-point/self-referential hashing;
- excluding any field other than `plan_sha256` from the payload;
- parsed-object reserialization with non-canonical separators or key ordering;
- treating the advisory digest as a control-plane `OperationId`, ticket, lease, handoff, gate or wave
  identity.

This supplement is normative with `TICKET_ISSUANCE_PLANNER.md` and
`swarm/ticket-issuance-plan-schema.toml`. Any earlier wording that could be read as hashing the emitted
object including `plan_sha256` is superseded by this rule.
