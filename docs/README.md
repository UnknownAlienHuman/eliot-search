# Documentation map

| Directory | Contents |
|---|---|
| `architecture/` | Normative Architecture 8.4 implementation master. |
| `adr/` | Accepted implementation and product-boundary decisions. |
| `contracts/p00/` | Bounded field-level contract projection, recipes, reasons and ports. |
| `config/` | Product configuration contracts and section ownership. |
| `current/` | Current-workspace, observation and overlay contracts. |
| `client/` | Standalone client and optional provider-edge contracts. |
| `evaluation/` | Product and release evaluation contracts. |
| `optional/` | Disabled-by-default optional depth profiles. |
| `handoff/` | Historical/package planning and dependency notes; not implementation authority. |
| `audit/` | Dated observations; they accept and authorize nothing. |
| `execution/` | Historical run records and diagnostics; not product architecture. |
| `generated/` | Generated product schemas/descriptors after their owning contracts are accepted. |

## Authority

Use this order:

1. Architecture Part I;
2. accepted product ADRs, including ADR 0005;
3. accepted public product contracts;
4. nearest package instructions and current issue/PR;
5. historical planning material.

`swarm/**`, `docs/handoff/**` and `docs/execution/**` do not authorize or block implementation. In
particular, `swarm/launch-state.toml`, ticket drafts and lease records are legacy/advisory coordination
metadata, not a current permission system.

## Standalone boundary

ELIOT Search is independently installable and runnable. ELIOT Memory OS and ELIOT Swarm Controller are
external consumers/controllers, not Search runtime dependencies. Search integration with ELIOT remains a
typed leaf adapter over the same standalone state and query owners.

## Qualification

External-artifact and product-evidence inputs live under repository-root `qualification/`. A packet or
captured diagnostic is not a passing receipt. Capabilities remain disabled until the exact accepted probes
run at the exact product revision and receive independent review.
