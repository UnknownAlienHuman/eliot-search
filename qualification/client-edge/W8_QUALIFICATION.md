# W8 generic client edge qualification contract

**Status:** `UNQUALIFIED`  
**Architecture:** ELIOT Search 8.4 S26, S32–S34, S37/G4, H16, P14  
**Platform baseline:** Windows x64 local provider transport  
**Optional profiles:** ELIOT and Research disabled unless separately admitted

## 1. Entry prerequisites

Generic-edge execution starts only after accepted:

- W0 contracts/domain/ports API digests;
- W1 runtime/control/protocol shell;
- W2 source/revision/readback contracts;
- W3 index/publication/pin contracts when indexed profiles are exercised;
- W4 access/planner/executor/validation/result/handle/continuation contracts;
- W5/W6 currentness/proof contracts for corresponding recipes;
- W7 revocation/purge/restore/retention hardening;
- exact configuration and fixture digests for this run.

Missing prerequisites yield `UNAVAILABLE`, not a partial G4 pass.

## 2. Generic qualification sequence

1. Start one daemon under an exact data-root owner and accepted runtime profile.
2. Create an isolated local transport and verify ACL/peer observations without treating them as auth.
3. Execute mutual hello/pairing/binding, including replay/expiry/role/incarnation negative cases.
4. Publish a binding-filtered capability descriptor and run hidden-membership noninterference fixtures.
5. Mint a bounded server-side standalone grant and prove requested scope cannot widen authority.
6. Execute frame/version/sequence/in-flight/progress/terminal/cancel/disconnect fixtures.
7. Execute typed generic request round trips across representative navigation, comparison, exact proof and
   `expand_handle@1` paths.
8. Revoke access/binding and apply purge/view/residency changes at every expansion/emission checkpoint.
9. Execute a client-owned immutable evidence snapshot/import fixture and prove Search returns no admission
   or completion disposition.
10. Run standalone CLI dependency/render/exit/redaction fixtures.
11. Audit logs, metrics, traces, errors and crash artifacts for content/token/hidden-scope leakage.
12. Publish immutable raw outputs and independent reviewer receipt.

## 3. Pairing and binding evidence

Required evidence proves:

- named-pipe ACL/local user is insufficient without pairing proof;
- challenge entropy, single use, TTL, peer role, peer identity and installation incarnation binding;
- replay, role substitution, stale incarnation and expired challenge rejection;
- binding durable commit/live snapshot acknowledgement order;
- mutation timeout recovery by operation identity;
- binding revocation order: durable → live snapshot → connection drain → request cancellation → dependent
  handle/continuation invalidation → acknowledgement;
- foreign/revoked binding receives no membership existence detail.

## 4. Capability descriptor evidence

For two bindings with disjoint scopes, changing hidden sources/memberships/readiness/index state must not
change the other binding's descriptor bytes/digest or content-free counts beyond accepted global daemon
health fields.

The descriptor must contain only visible opaque membership/readiness records and permitted recipe/profile
sets. A stale descriptor must not bypass current grant/access/profile validation.

Optional profile handler and descriptor availability must be coherent at every committed generation.

## 5. Generic request and handle evidence

Required round trips:

- `locate@1` or `find_text@1` to bounded validated candidate cards;
- a comparison/currentness-aware recipe when its profile is available;
- `compile_exact_scan@1`/`execute_exact_scan@1` with truthful proof coverage;
- `expand_handle@1` for excerpt, metadata, provenance and continuation classes.

Every result is checked for request/plan/view/security/profile identity, bounded coverage/freshness/
assurance/reasons and absence of client disposition or vendor state.

Handle expansion is tested against foreign binding, expired token, grant revocation, owner/view drift,
residency loss, purge and restrictive change immediately before emission. Possession alone never grants
access.

## 6. Client-owned evidence fixture

The fixture expands an authorized immutable source handle, independently verifies bytes/digest, writes a
client-owned immutable evidence record under fixture governance and preserves Search provenance.

Search must not:

- decide client admission/trust/finish;
- write the client's canonical store;
- delete client-owned evidence after later Search purge;
- expose a reverse write/credential channel.

Later Search revocation is represented as a client influence/revocation notice only.

## 7. Standalone CLI evidence

The CLI must use only provider protocol/public contracts. Dependency and runtime tracing prove it never
opens redb/CAS/Qdrant/secret storage.

Human and JSON outputs preserve material gaps, ambiguity, denominator kind, assurance, freshness and
reason codes. Partial/degraded outcomes have explicit non-success machine status/exit semantics.

## 8. Optional ELIOT profile

Run only when the feature/config/profile/binding prerequisites are explicitly accepted.

Evidence covers:

- exact WorkScope/disclosure never-widens mapping;
- exact SourceView/WorkspaceViewRevision/StateFence mapping and drift rejection;
- capability pulse filtering;
- result preservation of candidates/coverage/freshness/assurance/reasons;
- absence of memory/admission/finish disposition;
- provider failure narrowing without unrelated-work failure;
- no canonical credentials, store dependency or reverse write channel;
- Search core API digest unchanged by enabling the leaf.

A disabled adapter is reported `DISABLED`, not `PASS` and not a baseline failure.

## 9. Optional Research export profile

Run only when explicitly accepted and enabled.

Evidence covers:

- exact `eliotr.normalized.v1` manifest and canonical body digest;
- exact retained native readback and independent wire SHA-256;
- unknown load-bearing field rejection;
- unsaved/current-path/Qdrant-payload substitution rejection;
- federated/import modes without ownership transfer/cutover receipt;
- cutover mode with exact completed owner-transfer receipt;
- path traversal/absolute/device/duplicate/symlink/hardlink/reparse negative corpus;
- entry/byte/depth bounds and cancellation cleanup;
- timeout before/after final publication readback recovery;
- purge/revocation during export blocking publication;
- no cross-residency dedup/key reuse or research conclusion.

## 10. Evidence record

Every probe result contains:

```yaml
probe_id:
result: PASS | FAIL | UNAVAILABLE | DISABLED
repository_commit:
public_api_digests:
configuration_digest:
fixture_digest:
platform_and_transport_identity:
exact_command_or_harness:
started_at:
finished_at:
raw_output_ref:
raw_output_sha256:
reviewer_receipt_ref:
```

`PASS` requires non-empty immutable raw output and independent reviewer receipt. `DISABLED` is legal only
for optional profiles. Prose-only evidence is rejected.

## 11. Stop conditions

Generic G4 cannot pass if any of these exists:

- unauthenticated descriptor/request/expansion path;
- pairing replay or binding revocation race;
- descriptor hidden-scope/name/count leakage;
- client requested scope or adapter mapping widens authority;
- result carries client disposition/completion/reusable authorization;
- client/adapter direct redb/CAS/Qdrant/secret access;
- handle expansion lacks live reauthorization;
- silent continuation refresh to a newer fence;
- protocol limit/sequence/cancel/terminal violation;
- optional handler/descriptor incoherence;
- default telemetry leaks query/source/token/hidden-scope data;
- missing raw evidence/reviewer.

Optional profile failure disables that profile and must not invalidate an otherwise accepted generic
standalone edge.
