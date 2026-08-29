# W8 client-edge settings 1.0

Machine schema: [`../../config/w8-client-edge.toml`](../../config/w8-client-edge.toml).

## 1. Scope

This packet defines future P14 settings for the generic local provider edge, standalone CLI and optional
ELIOT/Research leaf profiles. It is schema-only and does not extend the central configuration registry
or authorize W8.

## 2. Field modes

- `LOCKED` — architecture/security/authority invariant; any override is rejected.
- `TUNABLE` — bounded resource or presentation setting validated by its owner.
- `QUALIFIED_REF` — immutable accepted feature/profile/mapping receipt, not free-form version text.
- `OPAQUE_SECRET_REF` — OS-bound reference only; plaintext is invalid.

A lower-precedence layer cannot unlock a field. Unknown sections/keys and unknown load-bearing fields
fail closed.

## 3. Generic edge

Mutual authentication, pairing proof, binding-filtered capability projection, no reverse authority and
no direct store access are locked. A client may tune only finite challenge/binding/descriptor resource
ceilings.

Reducing binding limits is a security-barrier change: excess or out-of-policy bindings are explicitly
revoked and dependent handles/continuations are invalidated before acknowledgement.

Capability descriptor TTL controls refresh planning only. A cached descriptor never becomes an
authorization permit and cannot suppress per-request live checks.

## 4. Standalone CLI

Output/progress presentation may be selected. CLI cannot open stores, silently hide partial coverage or
return exit code zero for a materially partial/degraded result when the command contract requires an
explicit partial status.

The standalone grant TTL is a server-side ceiling for a local authenticated binding. The CLI does not
mint or sign its own grant.

## 5. Optional ELIOT profile

The adapter defaults disabled. Activation requires:

```text
compiled feature
+ explicit config enablement
+ accepted generic-edge receipt
+ accepted ELIOT mapping fixture/profile receipt
+ current binding authorization
```

Canonical credentials, reverse writes, memory/admission/finish dispositions and fail-open behavior are
locked false. Configuration cannot create a new ELIOT core authority surface.

## 6. Optional Research export profile

The adapter defaults disabled and the wire protocol is locked to `eliotr.normalized.v1` with canonical
manifest-body SHA-256:

```text
3a5f9fd2b254eebe574b2c4a28f9804df0da9df359e59ceee125fa7da90fef22
```

Only finite bundle entry/byte ceilings are tunable. Unsaved export, implicit ownership transfer,
cutover without receipt, unknown load-bearing fields, path traversal and cross-residency dedup/key reuse
are locked out.

A lower bundle ceiling applies to new exports. An in-progress export exceeding the new ceiling is
cancelled before publication or fails with an explicit incomplete temporary-artifact cleanup receipt; it
is never silently truncated into a valid bundle.

## 7. Composite activation and rollback

Optional activation is `GATE_REQUIRED + RESTART_DEPENDENCY`. Every prerequisite receipt must be present
before daemon composition publishes the profile as available. If startup/mapping self-test fails, the
profile remains disabled and the generic standalone edge continues truthfully degraded.

The previous effective configuration remains authoritative until the optional profile has passed
startup, mapping/export fixture and capability descriptor publication. A mixed state—descriptor says
available while handler is absent, or handler active while descriptor hides it—is forbidden.

## 8. Redaction

Ordinary diagnostics may expose section/key, owner, provenance layer, reason code, boolean availability
and content-free counts. They exclude pairing proof, binding secret material, handle/continuation token,
Search grant, source/query content, raw paths, ELIOT canonical credentials/identifiers beyond the
accepted mapping, and Research bundle bytes.

## 9. Required settings tests

- every field has one owner, type, mode, default and bounded action where applicable;
- locked field override fails in file/environment/CLI layers;
- descriptor TTL never bypasses current request authorization;
- binding-limit reduction publishes revocation/invalidation before acknowledgement;
- standalone CLI cannot enable direct store access or partial-success exit zero;
- optional adapters remain disabled without feature/config/gate/profile/binding closure;
- ELIOT authority/credential/reverse-write fields cannot be enabled;
- Research protocol/digest, unsaved, ownership, cutover, unknown-field and path-safety floors cannot be
  weakened;
- failed optional-profile activation preserves the previous descriptor/config snapshot;
- redacted diagnostics contain no token, secret, source/query or export content.
