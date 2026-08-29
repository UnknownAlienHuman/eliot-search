# W8 generic client edge contracts 1.0

**Status:** implementation projection only; W8/P14 remains blocked.  
**Architecture:** ELIOT Search 8.4, S1.3–S1.4, S26, S32–S34, S37/G4, H16, P14.  
**Core rule:** Search returns bounded evidence candidates and proof reports; every client retains its own
interpretation, verification, admission, canonical-evidence and completion authority.

## 1. Actors and authority

```text
eliot-searchd
  ├─ generic local provider server
  ├─ source/index/query/lifecycle capability composition
  └─ optional leaf-profile activation after explicit gate/feature/config admission

eliot-search
  └─ thin standalone client over the same provider protocol

search-eliot-adapter                 optional leaf
  └─ ELIOT contract mapping only

search-research-export-adapter       optional leaf
  └─ immutable normalized-bundle export only
```

No client, adapter or CLI opens redb, Search CAS, Qdrant or the OS secret store. No client receives a raw
store handle, collection name, vendor filter, Qdrant cursor, point ID or reusable authorization
decision.

Search authority is limited to its admitted source namespaces, immutable revisions, provider planning,
candidate retrieval/validation and local lifecycle. Search does not own the client's task, memory,
verification, admission, synthesis, canonical evidence or finish state.

## 2. Protocol and recipe closure

The generic edge uses the P00 `ProviderEnvelope`, `SearchReadGrantClaims`, `SearchRecipeRequest` and
`RecipeResultV1` contracts. `RecipeIdV1` remains exactly:

```text
locate@1
find_text@1
inspect_entity@1
compare_implementations@1
explore_entity@1
corpus_profile@1
corpus_delta@1
provenance@1
compile_exact_scan@1
execute_exact_scan@1
expand_handle@1
```

W8 adds no recipe alias, adapter-specific core recipe or unversioned command. Optional adapters map to
these contracts or to their own leaf export protocol; they never extend Search core authority.

Frames remain `u32 little-endian length + UTF-8 JSON`, bounded by the accepted P00 protocol ceiling.
Baseline has no compression and no fragmented-message assembly.

## 3. Pairing and binding lifecycle

A named-pipe ACL, local user identity or loopback address is not sufficient authentication. A connection
must complete a mutually authenticated hello and pairing proof before any capability descriptor,
grant, request, handle expansion or cancellation is accepted.

```yaml
BindingRecord:
  binding_id: BindingId
  installation_id: InstallationId
  installation_incarnation_id: InstallationIncarnationId
  peer_role: standalone_cli | client_adapter
  peer_identity_digest: Blake3Digest32
  pairing_generation: NonZeroRevision
  permitted_profile_ids: bounded_set<ProfileId>
  disclosure_ceiling_ref: OpaqueRef
  issued_at: UtcTimestamp
  expires_at: UtcTimestamp | null
  revocation_generation: NonZeroRevision
  status: ACTIVE | REVOKED | EXPIRED
```

Binding records contain no canonical client credentials or client database access. Binding possession
does not grant corpus access; each request still carries a current bounded Search grant.

### 3.1 Pairing state machine

```text
UNPAIRED
  → PAIRING_CHALLENGE_ISSUED
  → PEER_PROOF_VERIFIED
  → BINDING_COMMITTED
  → ACTIVE

ACTIVE → REVOKED | EXPIRED
```

A challenge is single-use, TTL-bounded, peer-role-bound and installation-incarnation-bound. Challenge
replay, role substitution, incarnation mismatch or proof reuse fails closed. Binding acknowledgement
occurs only after the durable binding state and live revocation snapshot are observable.

### 3.2 Reconnect

Reconnect creates a new authenticated connection state. It never reuses connection sequence or
in-flight request guards. Existing provider-local durable handles remain subject to binding and live
reauthorization; connection-scoped requests, progress streams and ephemeral cancellation capabilities do
not survive disconnect.

## 4. Connection and request state

```yaml
ConnectionState:
  connection_id: OpaqueId
  binding_id: BindingId
  negotiated_protocol: ProtocolVersion
  peer_role: standalone_cli | client_adapter
  last_connection_sequence: u64
  in_flight_request_ids: bounded_set<RequestId>
  accepted_capability_digest: Blake3Digest32
  state: AUTHENTICATED | DRAINING | CLOSED
```

Connection sequence is strictly monotonic. Duplicate/regressed/replayed envelopes are rejected before
request admission. At most the accepted in-flight ceiling is active per connection.

Each request has exactly one lifecycle:

```text
RECEIVED
  → ADMITTED
  → PLANNING
  → RETRIEVING
  → VALIDATING
  → PROJECTING
  → RESULT | ERROR | CANCELLED
```

Progress event sequence is monotonic. Exactly one terminal event is emitted. Protocol code does not
truncate, reinterpret or upgrade coverage; it transports the already validated bounded recipe result.

Cancellation is idempotent. Cancellation/disconnect propagates to request-local work and releases
request-owned route/epoch pins. If a mutating server command may already have committed, its owning
package resolves the outcome through durable operation identity/readback; the protocol cannot fabricate
rollback.

## 5. Binding-filtered capability descriptor

After authentication, the daemon composes an authoritative capability snapshot and the protocol
projects a binding-filtered `SearchProviderCapabilityDescriptor`.

Filtering rules:

- only memberships and source-owner generations visible to the current binding are represented;
- inaccessible membership names, paths, corpus names, counts and readiness are not included or inferred;
- supported recipes are intersected with binding/grant/profile policy;
- optional profile states are reported only when the binding may know the profile exists;
- readiness and degraded reasons are content-minimized and scoped to visible opaque memberships;
- descriptor availability never grants access or client authority;
- descriptor digest binds protocol, owner epoch, access generation, inventory revision, visible epoch,
  route revision and visible membership readiness.

A client uses the descriptor for planning only. Each request is independently validated against current
live state. A stale descriptor narrows or rejects the request; it never widens access.

## 6. Grants and requested scope

A client may request a scope and budget class. The request is not authority.

```text
requested scope
  ∩ binding policy
  ∩ current SearchReadGrant
  ∩ authoritative membership/source view
  ∩ live deny/purge/shadow state
  = executable authorized scope
```

The adapter or CLI cannot sign a grant, add memberships, raise disclosure ceilings or choose a vendor
filter. Standalone mode obtains a bounded server-minted local-user grant. Managed client profiles use
their accepted binding/grant integration but Search still validates every claim.

Grant and scope failures are ordinary typed results. They do not reveal whether a foreign membership or
handle exists.

## 7. Generic request/response round trip

```text
authenticated binding
→ binding-filtered capability descriptor
→ typed recipe request + current grant + explicit source view/scope
→ server-owned plan and execution
→ exact source-backed validation
→ bounded RecipeResultV1
→ client-owned interpretation/verification/admission
```

Every result binds request/plan identity, relevant source/view/security/profile generations, truthful
coverage/freshness/assurance, reason codes and provider-local handles where eligible.

A result never contains:

- a client memory/admission/finish disposition;
- a claim that the client task is complete;
- canonical client evidence identity;
- a reusable authorization permit;
- raw source bytes beyond an authorized bounded handle expansion;
- raw Qdrant/redb/protocol-internal state.

Provider failure narrows coverage and returns typed degradation. It does not block unrelated client work
or instruct a client to admit weak evidence.

## 8. Handle and continuation expansion

All provider-local expansion uses `expand_handle@1`. The public handle token is opaque and non-self-
describing. Possession grants no access.

Before expansion, Search revalidates:

- authenticated binding and current grant;
- handle/continuation status, TTL and quota;
- owner generation and exact source/workspace view;
- current access/live-deny/purge/shadow state;
- residency/retention authorization;
- exact retained revision or authenticated unsaved snapshot eligibility;
- requested expansion kind/range/disclosure/byte ceiling;
- route/epoch/profile fence for continuations.

Immediately before emission, Search rechecks restrictive state. Revocation/purge during expansion blocks
emission. A stale or foreign token returns a binding-safe error without existence disclosure.

Ephemeral continuation disconnect/restart releases its process-local pin and cannot silently continue
against a new corpus. Durable replan checkpoints are explicit-job-only and own no process-local pin or
unsaved bytes.

## 9. Client-owned evidence snapshot, pin and import

Search handles are navigation capabilities, not canonical client evidence. A client that needs durable
load-bearing evidence must use its own governance to snapshot, pin or import the immutable source bytes
and Search receipt.

The generic fixture verifies:

1. exact source-handle expansion under current authorization;
2. client recomputation of source/excerpt digest;
3. client-owned immutable evidence record creation;
4. preservation of Search source namespace/owner generation/revision/view provenance;
5. later Search revocation/purge notification represented as a client influence/revocation event, not a
   Search-side delete of client canonical data.

No new Search recipe is introduced for client canonical admission. Search cannot determine whether the
client should trust, remember or finish from the evidence.

## 10. Standalone CLI profile

`eliot-search` is a thin client of the same daemon and protocol. It owns argument parsing, session setup,
typed request construction, progress/result/error rendering and exit status only.

It never opens Search stores or Qdrant. A command family is exposed only when the binding-filtered
capability descriptor and current daemon profile support it.

Standalone output must preserve:

- partial/degraded coverage and reason codes;
- ambiguity and exact-proof limitations;
- source-handle opacity;
- explicit refresh behavior for expired continuations;
- non-zero/typed exit status for usage, protocol, authorization, partial, unavailable and failed states.

Machine-readable output is the canonical bounded provider result plus CLI metadata; human output may
format but cannot omit material gaps or relabel coverage.

Managed and standalone daemons cannot simultaneously own one data root. A mode transition requires
owner drain, epoch fence and restart.

## 11. Optional ELIOT compatibility profile

The ELIOT adapter is a disabled-by-default leaf. It maps existing ELIOT provider concepts to generic
Search contracts:

```text
ELIOT WorkScope + disclosure policy
  → requested Search scope + requested ceilings
  → server-authoritative grant intersection

ELIOT SourceView / WorkspaceViewRevision / StateFence
  → exact Search source view and request dependency fence

Search capability descriptor
  → local provider capability pulse

Search result
  → candidates + coverage + freshness + assurance + reasons + immutable refs
```

The adapter does not:

- receive canonical database credentials;
- open ELIOT or Search stores directly;
- mint Search authority or widen membership closure;
- create a new `eliot.search` core authority surface;
- return an ELIOT memory disposition, admission decision or finish decision;
- synchronously fail unrelated ELIOT work when Search is degraded.

Search remains mutable owner of each admitted local namespace. ELIOT stores immutable refs and governed
influence/evidence under its own rules. A source-owner cutover is a separate explicit protocol; ordinary
query/result mapping is never a cutover.

## 12. Optional Research normalized-bundle export

The Research adapter is a disabled-by-default leaf implementing exactly `eliotr.normalized.v1`. It is
not an online research orchestrator and returns no conclusions.

### 12.1 Eligibility

Export requires:

- immutable retained `SourceRevision` and selected materialization;
- current source owner/view/access/residency/retention/purge authorization;
- exact native readback and digest/length verification;
- disclosure and allowed-use compatibility;
- bounded export purpose and expiry;
- no unsaved overlay unless explicit snapshot admission already created a durable revision and residency
  receipt.

### 12.2 Wire identity

The adapter independently computes protocol-required SHA-256 over reopened native/normalized bytes. It
does not relabel internal BLAKE3 or hash Qdrant payload text.

The manifest body must match the architecture `eliotr.normalized.v1` shape and canonical body SHA-256:

```text
3a5f9fd2b254eebe574b2c4a28f9804df0da9df359e59ceee125fa7da90fef22
```

Unknown load-bearing fields fail closed. Optional fields are optional only where the protocol declares
them optional.

### 12.3 Ownership modes

```text
federated_reference
immutable_import
ownership_cutover
```

Ordinary export produces an immutable import/reference candidate and transfers no source ownership.
`ownership_cutover` is valid only after the separate source-owner cutover completed. The export verifies
a receipt binding old owner generation, exact source/view fence, identity mapping, new owner generation
and activation. The receipt field is absent for every other mode.

### 12.4 Bundle assembly

The logical bundle contains only registered relative paths (`manifest`, `content.md` and qualified
optional structure/mapping/table artifacts). Traversal, absolute paths, device names, symlink/hardlink
escape, duplicate normalized paths and unbounded entry counts/bytes are rejected.

Container/archive format is an independently replaceable leaf implementation and must preserve exact
logical file bytes and manifest digests. The adapter cannot perform cross-residency-domain CAS dedup or
key reuse.

## 13. Reconnect, retry and idempotency

Read requests are not stored as durable jobs by default. Losing a read result requires a new request ID,
unless an explicit protocol operation is documented as replay-safe and the daemon still owns its
request state.

Mutating binding, pairing, export and doctor operations use stable operation identities in the owning
package's bounded idempotency store. Same identity plus same canonical input reconstructs the accepted
receipt. Same identity plus different input is rejected.

A transport timeout after a mutation may have committed. Recovery reads the owning durable state by
operation identity; the protocol/adapter never retries blindly under a new identity.

## 14. Leakage and noninterference

Default protocol logs/metrics/traces contain only opaque operation/binding/request IDs, reason codes,
counts, duration, resource use and profile identities. They exclude:

- source or unsaved content;
- raw query text;
- absolute paths;
- API/pairing/handle tokens;
- inaccessible membership/corpus names;
- ELIOT canonical identifiers not explicitly permitted by the leaf mapping;
- Research export content.

Adding an inaccessible membership or optional profile must not change another binding's descriptor,
result ordering, counts, facets, progress, timing beyond accepted leakage tolerance or error detail.

## 15. Configuration interaction

W8 settings are defined in `config/w8-client-edge.toml` and
`docs/config/W8_CLIENT_EDGE_SETTINGS_1.0.md`.

Security/authority fields are locked. Bounded transport/session/descriptor/export resource ceilings may
be tunable. Optional profiles require all of:

- compiled Cargo feature;
- explicit configuration enablement;
- accepted generic-edge handoff;
- accepted profile mapping/export fixture receipt;
- current binding authorization.

Enabling a profile is `GATE_REQUIRED` plus dependency restart. Configuration alone cannot load or
authorize an optional adapter.

## 16. Failure ownership

| Failure | Owner |
|---|---|
| frame/session/sequence/pairing/binding | `search-provider-protocol` |
| authoritative capability input/composition/profile activation | `eliot-searchd` |
| CLI parse/render/exit mapping | `eliot-search` |
| ELIOT scope/view/result/authority mapping | `search-eliot-adapter` |
| normalized bundle/digest/ownership/export assembly | `search-research-export-adapter` |
| grant/scope/live deny | `search-access` |
| handle state/expansion | `search-handles` or `search-continuation` |
| source readback/residency | owning source/revision packages |

Consumers preserve producer reason codes and do not reinterpret them into success.

## 17. G4 exit evidence

Mandatory generic-edge evidence:

- frame/JSON/version/sequence/in-flight/progress/terminal/cancel/disconnect fixtures;
- pairing replay/role/incarnation/expiry and binding revocation fixtures;
- grant/scope never-widens and foreign-membership non-disclosure;
- capability descriptor binding filtering and noninterference;
- generic typed request → server plan → validated candidate/proof round trip;
- handle expansion reauthorization at access/revocation/purge/view/residency checkpoints;
- client-owned evidence snapshot/import fixture showing no Search admission authority;
- standalone CLI no-store dependency and truthful partial/degraded rendering;
- default telemetry/trace leakage audit.

Optional profile evidence, only when enabled:

- ELIOT scope/view/fence/result mapping and no reverse authority/channel;
- Research exact manifest/digest, native-readback, ownership-mode, unsaved rejection and path-safety
  fixtures.

All evidence records bind exact code/API/config/fixture identities and immutable raw outputs. A profile
remaining disabled is not a failure of standalone baseline G4.

## 18. Stop conditions

W8 remains blocked on any of:

- unauthenticated descriptor, request or handle expansion path;
- binding or grant widening by adapter/CLI request;
- inaccessible membership/name/count disclosure;
- capability availability treated as authorization;
- result containing client admission/memory/finish disposition;
- raw Search/Qdrant/redb access from any client package;
- handle expansion without live reauthorization;
- silent continuation refresh against a new fence;
- optional profile enabled without feature/config/gate receipt;
- ordinary export transferring ownership;
- unsaved or purged material entering a durable Research bundle;
- wire digest relabeled from an internal digest;
- reverse write/credential channel into ELIOT;
- missing raw evidence or reviewer receipt.
