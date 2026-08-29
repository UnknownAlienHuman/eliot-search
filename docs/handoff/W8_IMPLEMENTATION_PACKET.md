# W8 generic client edge implementation packet

**Stage:** P14 / Gate G4  
**Status:** contract-ready, implementation-blocked  
**Launch authority:** `swarm/launch-state.toml`

W8 exposes the already accepted Search capabilities through one authenticated, bounded, generic local
provider edge. It does not add a Search recipe or give clients authority over source membership,
authorization, evidence admission, memory, synthesis, task completion or finish state.

## Package boundaries

| Package | W8 responsibility | Bounded packet |
|---|---|---|
| `search-provider-protocol` | pairing, binding, connection/request lifecycle, binding-filtered capability projection | `crates/search-provider-protocol/W8_HARDENING.md` |
| `eliot-searchd` | generic-edge composition, authoritative capability inputs, standalone grant minting, optional-profile activation | `bins/eliot-searchd/W8_INTEGRATION.md` |
| `eliot-search` | thin standalone client, truthful rendering and exit status | `bins/eliot-search/FUNCTIONS.md` |
| `search-eliot-adapter` | optional ELIOT scope/view/result mapping with authority guard | `crates/search-eliot-adapter/FUNCTIONS.md` |
| `search-research-export-adapter` | optional retained-materialization export as `eliotr.normalized.v1` | `crates/search-research-export-adapter/FUNCTIONS.md` |

Exact per-writer read sets are in `swarm/w8-client-edge.toml`. The cross-package authority and lifecycle
contract is `docs/client/W8_GENERIC_CLIENT_EDGE_CONTRACTS_1.0.md`.

## Implementation order

```text
accepted W0-W7 handoffs and G3/W7 receipts
    ↓
search-provider-protocol W8 hardening
    ↓
eliot-searchd generic-edge composition
    ├─ eliot-search standalone CLI
    ├─ optional search-eliot-adapter
    └─ optional search-research-export-adapter
```

The protocol package may be implemented/reviewed before daemon integration. The CLI starts only from an
accepted protocol handoff. Optional leaf adapters start only after the generic edge is accepted and their
feature/config/profile/binding activation closure is complete.

## Generic-edge acceptance

The mandatory baseline proves:

- mutual hello, pairing proof and binding lifecycle beyond pipe ACL/local-user checks;
- sequence/replay/frame/in-flight/progress/terminal/cancel/disconnect bounds;
- server-minted bounded grant whose requested scope never widens authority;
- binding-filtered capability descriptor and hidden-scope noninterference;
- exact eleven-recipe closure;
- typed request → server plan → validated candidate/proof result round trips;
- handle expansion with live binding/grant/owner/view/residency/purge reauthorization;
- client-owned evidence snapshot/import without Search admission or canonical-store authority;
- standalone CLI with no direct store/index path and truthful partial/degraded rendering;
- content/token/hidden-scope leakage audit.

A capability descriptor is planning data, not a permit. Every request and handle expansion revalidates
current server authority.

## Optional ELIOT leaf

The ELIOT profile is disabled by default and not required for standalone G4. When enabled, it maps only
existing generic contracts. It cannot:

- receive canonical credentials or open ELIOT/Search stores;
- mint grants or widen membership/disclosure;
- add a core Search recipe or `eliot.search` authority surface;
- return memory/admission/finish disposition;
- create a reverse write channel;
- transfer mutable source ownership through an ordinary query/result mapping.

Provider degradation narrows coverage and must not fail unrelated ELIOT work.

## Optional Research export leaf

The Research profile is disabled by default and not required for standalone G4. When enabled, it:

- reopens the exact retained native revision/materialization;
- verifies native identities/lengths and computes wire SHA-256 independently;
- emits exactly `eliotr.normalized.v1` with canonical manifest-body digest;
- rejects unsaved/current-path/Qdrant-payload substitution;
- validates disclosure/residency/retention/purge and ownership mode;
- treats ordinary export as immutable reference/import, not owner transfer;
- accepts `ownership_cutover` only after a separately completed exact cutover receipt;
- rejects traversal, absolute/device, duplicate, symlink/hardlink/reparse and unbounded bundle layouts;
- recovers timeout around final publication by exact bundle readback or unpublished-temp cleanup.

## Configuration

`config/w8-client-edge.toml` separates `LOCKED`, bounded `TUNABLE` and immutable `QUALIFIED_REF` fields.
Optional activation requires compiled feature, explicit config, accepted generic-edge receipt, accepted
profile fixture and binding authorization. Configuration alone never activates a profile.

The daemon publishes handler routing and capability availability atomically. Failure preserves the prior
profile set and keeps the generic standalone edge available with truthful degradation.

## Qualification

Future evidence lives under `qualification/client-edge/`. Generic probes begin `UNAVAILABLE`; optional
profile probes begin `DISABLED`. `PASS` requires immutable raw output and independent review. The gate
mapping is `qualification/client-edge/gate-map.toml` and preserves the existing G4 evidence IDs.

## Hard stop conditions

- unauthenticated descriptor/request/expansion;
- pairing replay or revocation acknowledgement before dependent invalidation;
- hidden membership/name/count/reason leakage;
- requested scope or adapter mapping widening authority;
- result containing client disposition/completion/reusable authorization;
- client/adapter direct redb/CAS/Qdrant/secret-store path;
- handle expansion without final live recheck;
- silent continuation refresh against newer state;
- optional profile active without complete activation closure;
- ELIOT reverse authority/credential/write path;
- ordinary Research export transferring ownership or exporting unsaved/purged content;
- internal digest relabelled as wire SHA-256;
- missing raw evidence/reviewer receipt.

## Honest status

No protocol binding, CLI, ELIOT adapter or Research export Rust implementation is added by this packet.
No G4 probe has executed. W8 remains blocked until accepted dependency handoffs and explicit
integration-owner tickets exist.
