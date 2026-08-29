# W8 integration contract — `eliot-searchd`

The daemon is the composition root for the generic client edge. This file adds no capability logic to
`main`; it specifies wiring, startup order and coherent availability publication.

## `compose_generic_client_edge`

```text
compose_generic_client_edge(
  runtime_owner,
  control_and_live_snapshots,
  protocol_server,
  access_compiler,
  planner_executor_projector,
  handle_and_continuation_ports,
  capability_sources,
  config,
  accepted_receipts,
) -> Result<GenericEdgeRuntime, DaemonError>
```

Requires accepted W0–W7 package/API/configuration receipts and no fail-closed lifecycle/security domain.
The daemon binds vendor-neutral ports only; clients receive no concrete store/index/process object.

## `build_authoritative_capability_snapshot`

```text
build_authoritative_capability_snapshot(control, readiness, profiles, lifecycle, observation)
  -> Result<AuthoritativeCapabilitySnapshot, DaemonError>
```

Collects current owner epoch, source owner generations, recipe/profile availability, visible epoch,
route/access/inventory revisions, observation freshness, membership readiness and degraded reasons.

The snapshot is complete server-side state, not client output. It is passed to the protocol for
binding filtering. Daemon code must not precompute one global unfiltered descriptor and reuse it across
bindings.

## `mint_standalone_grant`

```text
mint_standalone_grant(local_binding, requested_scope, policy, clock, mutation)
  -> Result<SearchReadGrantClaims, DaemonError>
```

Uses current server membership/access/disclosure policy and a bounded TTL. Requested scope can only
narrow the authoritative closure. The CLI never signs or chooses memberships itself.

Same operation identity plus same binding/scope/policy generation reconstructs the receipt. A policy or
binding generation change requires a new operation/request.

## `activate_optional_profile`

```text
activate_optional_profile(profile_kind, feature_receipt, config, fixture_receipt, binding_policy)
  -> Result<OptionalProfileActivationReceipt, DaemonError>
```

For ELIOT or Research profiles, requires:

```text
compiled Cargo feature
+ explicit config enablement
+ accepted generic-edge receipt
+ accepted profile fixture/mapping/export receipt
+ current binding authorization policy
```

Activation builds the leaf handler, runs startup self-test, then atomically publishes handler routing
and capability snapshot generation. Configuration alone is insufficient.

Failure preserves the previous descriptor/handler set and reports the optional profile disabled; it
does not fail the standalone generic edge.

## `deactivate_optional_profile`

```text
deactivate_optional_profile(profile_kind, reason, mutation)
  -> Result<OptionalProfileDeactivationReceipt, DaemonError>
```

Stops new profile requests, drains/cancels bounded in-flight leaf work, invalidates profile-scoped
capability descriptors and publishes one coherent unavailable state. It does not invalidate generic
source handles unless their owning security/lifecycle state requires it.

## `serve_provider_connection`

```text
serve_provider_connection(transport, protocol_runtime, capability_source, request_router, shutdown)
  -> Result<ConnectionReceipt, DaemonError>
```

The daemon provides authoritative capability inputs and routes already authenticated/bounded requests.
It does not duplicate framing, grant, planning, handle or adapter mapping logic.

On disconnect it propagates cancellation and resource release. Mutations with unknown outcome remain
owned by their package recovery state machines.

## `drain_generic_edge`

```text
drain_generic_edge(reason, deadline) -> Result<GenericEdgeShutdownReceipt, DaemonError>
```

Stops new connections, publishes draining state to existing connections where permitted, cancels reads,
allows bounded mutation recovery/receipt completion, drains optional adapters and closes protocol
resources before releasing runtime owner state.

## Coherent availability invariant

At every published generation:

```text
descriptor says available ⇔ routed handler exists and passed accepted startup/fixture checks
descriptor says unavailable ⇒ no new request can reach that handler
```

There is no window where a client sees a profile enabled without a handler or can reach a hidden handler
not represented by its filtered descriptor.

## Forbidden composition

- raw redb/Qdrant/CAS/secret clients passed to CLI or adapters;
- client-specific authority inside contracts/domain/query packages;
- adapter self-registration or self-activation;
- one global unfiltered descriptor shared across bindings;
- optional profile failure taking down unrelated generic/DIRECT capabilities;
- mode transition without drain, owner-epoch fence and restart.

## Required tests

- accepted dependency/port graph only;
- per-binding descriptor construction, no global leakage cache;
- standalone grant scope-never-widens;
- optional activation missing each prerequisite;
- activation/deactivation crash before/after handler/descriptor publication;
- optional failure leaves standalone edge available and explicitly degraded;
- clients/adapters have no concrete store/index path;
- shutdown releases request-local resources and preserves mutation recovery ownership;
- managed/standalone same-root co-ownership rejected.
