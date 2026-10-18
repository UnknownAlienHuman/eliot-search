# Function contract — `eliot-search`

The CLI owns invocation capture, local connection assembly, request construction, bounded presentation
and exit status. `search-provider-client` owns the protocol/session engine. The source already exists;
the supported native provider path and installed qualification remain incomplete.

The [current architecture](../../docs/architecture/README.md) changes the target to an OS-owned local
connection and explicit source scope. Existing paired registration below is a source inventory for the
restricted-client path, not a prerequisite for child-stdio local search. Do not manufacture grant/proof
fields to satisfy the old constructor; simplify its real request/session contract when implementing.

## Current source and missing linkage

Verified at `c8e38b6fc9829f1cd38070773b1b3e5b21de18ad`; refresh the actual caller census before editing.

| Boundary | Delivered code / remaining caller |
|---|---|
| Default dispatch | [entry.rs](src/entry.rs) uses public/DIRECT dispatch; [public_client.rs](src/public_client.rs) connects to its owned daemon child through stdin/stdout. [app.rs](src/app.rs) rejects old address/token-file provider commands before its old parser. `legacy-loopback-harness` is off by default; its presence is not a live product fallback. |
| Typed session | [search-provider-client](../../crates/search-provider-client/) already owns the extracted session, framing, correlation and cancellation engine (#235/PR #383). CLI has API consumers; the missing result is the supported native command connection, not another extraction. |
| Trusted input | [NativeLocalRegistration](src/native_local_registered.rs) needs installation/incarnation, binding, peer digest, pairing generation, proof ref and optional capability digest from an independently trusted installation/configuration owner. Its constructor/resolver have only fixture callers; no product input producer is connected. |
| Client connection | `resolve_registered_local` selects the exact credential locator and retains the original deadline. [open_resolved_windows](src/provider_client/local.rs) consumes that handoff through the existing Windows `NativeLocalConnection` and `TypedProviderSession::open_native`; it has no caller. |
| Daemon listener | [StandaloneLocalOwner::bind](../eliot-searchd/src/access_composition/native_binding_listener/local.rs) accepts a platform factory, but has no concrete listener implementation/caller. Its server-side durable registration expectation is not a client registration source. |

The [existing Credential Manager adapter](src/native_credentials.rs) reads only the canonical
purpose-bound pairing locator. Possession of a key, a pipe name, an endpoint file or peer-asserted
installation coordinates cannot authorize a restricted client. Retain pairing/current binding checks
when that profile is selected. Trusted local stdio and OS-owned pipes use the local architecture.
Do not use the legacy `runtime/native-endpoint.v1` TCP descriptor as trusted installation input.

[#332](https://github.com/UnknownAlienHuman/eliot-search/issues/332) owns named input/listener/command
cutovers. Reuse these owners and establish the actual producer/consumer before deleting the replaced
path. A feature flip or another unused constructor cannot close that integration.

## Invocation and operation contract

Capture `args_os()` and relevant environment once, retain non-UTF-8 path values and pass a closed
invocation/config-source snapshot. Unknown/unversioned recipes, raw vendor filters, collection/point
IDs and direct store paths fail. Sensitive proof/credential values do not enter plaintext argv.

These are behavior roles, not mandatory new wrapper functions:

| Role | Required behavior |
|---|---|
| Parse/build | Capture one supported command and explicit scope/query/limits. The #213 evidence/orientation schemas already exist. Add their real serving path; all optional recipe handlers need not precede useful indexed search. |
| Connect/authenticate | One owned child-stdio or OS-authenticated local pipe connection. Reuse framing/session code; add pairing only for a selected restricted-client profile. One setup deadline and one absolute operation deadline; phases do not renew either. |
| Capabilities/scope | Advertise connected handlers. Resolve the user's explicit scope in the daemon. Restricted-client grants intersect real server policy; the CLI cannot mint or widen them. |
| Run/cancel | Use the existing typed request lifecycle, monotonic progress and one terminal result/error/cancellation. Cancellation is idempotent; acknowledge it only as the protocol permits. Disconnect or timeout cannot claim rollback of a possible server mutation. |
| Expand | Send an opaque handle/continuation with the active scope, limits and expiry; apply grants only in the restricted-client profile. Do not decode token contents, cache a permit or silently refresh expiry. |
| Close | Stop new requests, cancel bounded local work where possible and close the owned transport. Clean up a child only if this invocation created it, with bounded kill/wait and explicit unknown cleanup. Daemon shutdown requires an explicit authenticated server command. |

## Output, exits and privacy

Human and JSON/NDJSON output provide useful locations/ranges/snippets, exact revision, freshness/gaps
and continuation expiry. Preserve a denominator only when an explicit exhaustive operation supplies it. Never relabel
`candidate_scope` or `unknown` as `complete_scope`. External JSON uses closed serialized output
structures, bounded escaped records and one physical line per NDJSON record; changing an old wire shape requires changing both real ends/version, not silently redefining stored
identity bytes.

Keep the closed exit distinctions `SUCCESS`, `PARTIAL_OR_DEGRADED`, `USAGE_ERROR`,
`AUTHENTICATION_OR_BINDING_FAILED`, `UNAUTHORIZED_OR_SCOPE_EMPTY`, `PROTOCOL_ERROR`,
`DEPENDENCY_UNAVAILABLE`, `REQUEST_CANCELLED`, `REQUEST_FAILED` and `SNAPSHOT_EXPIRED`.
Material partial/degraded results never become silent success; any explicitly permitted
successful-with-warning contract retains a non-zero machine status field.

Preserve typed usage/unsupported-command, daemon-unavailable, pairing-required, revoked-binding,
version-mismatch, unavailable-capability, partial/cancelled/expired and output-limit failures.
Presentation/progress may be local; restricted-client grant TTL is only a requested ceiling. Keep secrets,
pairing proofs and source/query content out of default logs. Return actionable scoped errors and requested
snippets to the user. Help/status on an uninitialized
installation creates nothing. No direct redb/CAS/Qdrant access or arbitrary credential target.

## Removal boundary and evidence

The obsolete `provider_client/core.rs`, its four implementation submodules and `endpoint_client.rs`
are a six-file / 1,638-physical-line candidate. `remote_client.rs` and `app/indexed.rs` add 393 lines
of separate harness entry paths: eight files / 2,031 physical lines in total, counted as newline bytes.
These disjoint counts include comments/blank lines and are not achieved net production deletion.

Remove each last caller and dispose of its still-relevant fixtures in the same bounded cutover.
A harness-only deletion may proceed once the census proves its supported callers are already absent;
it need not wait for every native capability in #332.
Mixed `provider_client.rs` retains typed re-exports/local assembly; mixed `app/core.rs` retains DIRECT
commands. Neither is a whole-file deletion candidate. Retain paired bootstrap only for a real restricted-client consumer; an uncalled constructor alone does
not make it a required local baseline.

Retain causal argument-order/`--`/non-UTF-8, JSON escaping/limits, protocol sequencing,
slow-peer/cancel/EOF, child-cleanup, disclosure/partial-exit and opaque-handle cases. Pairing/binding/grant
cases apply when the restricted-client feature is retained or changed.
The [execution protocol](../../docs/product/EXECUTION_PROTOCOL.md#4-implement-then-verify-the-production-closure)
selects the changed production targets; test/native/installed evidence follows integrated code.
