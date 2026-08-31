# Ticket-issuance advisory plan digest v2

`plan_sha256` is not a self-hash and is not a control-plane operation or record identity.

It is computed as:

```text
SHA-256(
  ASCII("eliot-search/ticket-issuance-plan/v2\0")
  || canonical JSON bytes of the complete plan object with only `plan_sha256` omitted
)
```

Canonical JSON is UTF-8, LF terminated, lexicographically sorted by object key, compact and
non-pretty-printed. Array order is semantic and preserved.

The emitted object then adds `plan_sha256`. A caller may separately hash the complete emitted file, but
that value is distinct and is never embedded into the plan.

Implementations and validators reject:

- hashing a placeholder and replacing it;
- fixed-point or self-referential hashing;
- omitting any field other than `plan_sha256`;
- changing array order;
- non-canonical reserialization;
- treating the advisory digest as an operation ID, context, ticket, lease, handoff, gate or wave identity.
