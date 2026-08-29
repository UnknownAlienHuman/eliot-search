# Function contract — `search-eliot-adapter`

**Status:** optional disabled-by-default P14 compatibility profile; no implementation exists yet.

This leaf maps existing ELIOT provider contracts to generic Search contracts. It owns no ELIOT or
Search authority/state store and never changes core Search schemas.

## `validate_profile_activation`

```text
validate_profile_activation(feature, config, generic_edge_receipt, mapping_receipt, binding_policy)
  -> Result<EliotAdapterProfile, AdapterError>
```

Requires compiled feature, explicit enablement, accepted generic edge, exact mapping fixture/profile and
current binding authorization policy. Configuration alone is insufficient.

Failure returns profile disabled and does not affect standalone generic Search.

## `map_work_scope`

```text
map_work_scope(work_scope, disclosure_policy, capability_descriptor)
  -> Result<EliotRequestedSearchScope, AdapterError>
```

Produces requested Search scope/domain/ceiling only. It cannot mint a grant, add an invisible
membership, raise a disclosure ceiling or translate a capability into authority.

Unknown ELIOT scope variants/fields and stale descriptor identity fail closed.

## `map_source_view_and_fence`

```text
map_source_view_and_fence(source_view, workspace_view, state_fence, capability)
  -> Result<SearchViewAndDependencyRequest, AdapterError>
```

Preserves exact source view/workspace revision and relevant owner/access/inventory/route/epoch/profile
dependencies. It cannot substitute latest state, discard unknown dependencies or create a generic opaque
fence that hides missing load-bearing axes.

## `map_provider_request`

```text
map_provider_request(eliot_query_or_verify, requested_scope, view_fence, budget)
  -> Result<SearchRecipeRequest, AdapterError>
```

Maps only to the exact eleven generic recipes and accepted leaf compatibility labels. It does not add a
new `eliot.search` authority surface or send client-authored vendor filters.

Client task/admission/finish intent is not serialized into Search request authority.

## `map_capability_pulse`

```text
map_capability_pulse(binding_filtered_descriptor) -> EliotLocalProviderCapabilityPulse
```

Reports only visible generic capabilities, readiness, freshness and degraded reasons. It cannot expose
hidden memberships or interpret availability as permission/quality/admission.

## `map_search_result`

```text
map_search_result(result, original_request, mapping_profile)
  -> Result<EliotProviderResult, AdapterError>
```

Preserves candidates, immutable source refs/handles, coverage denominator, freshness, assurance,
validation state, ambiguity/conflicts, gaps and reason codes.

Output contains no ELIOT memory disposition, canonical admission, verification verdict beyond the exact
Search proof report, synthesis result, task completion or finish decision.

Search failure/degradation maps to narrower coverage/availability and does not synchronously block
unrelated ELIOT work.

## `map_search_revocation_event`

```text
map_search_revocation_event(event, imported_influence_refs)
  -> Result<EliotInfluenceRevocationNotice, AdapterError>
```

Maps Search source/purge/owner-generation revocation to a governed ELIOT influence/evidence notice. It
does not delete ELIOT canonical evidence or mutate ELIOT memory directly.

## `validate_no_reverse_authority`

```text
validate_no_reverse_authority(profile_graph, public_api, dependencies)
  -> Result<AuthorityBoundaryReceipt, AdapterError>
```

Proves:

- no ELIOT canonical DB/client or Search store/Qdrant dependency;
- no canonical credential or reverse write channel;
- no client-specific type imported into Search contracts/domain;
- no grant signing/membership mutation/source-owner transition;
- no memory/admission/finish disposition in output;
- generic Search contracts remain canonical.

## Source ownership

Search remains mutable owner of admitted local namespaces. ELIOT stores immutable refs and governed
influence records. Ordinary mapping cannot transfer ownership. A cutover uses the separate source-owner
protocol and accepted receipt outside this adapter request/result path.

## Idempotency and cancellation

Pure mapping functions are deterministic. Profile activation/deactivation uses daemon operation identity.
Cancelling a Search request follows generic protocol semantics; the adapter does not fabricate ELIOT
rollback or retry under a widened/new scope.

## Typed failures

- `PROFILE_DISABLED`
- `CLIENT_ADAPTER_AUTHORITY_VIOLATION`
- `ELIOT_MAPPING_PROFILE_MISMATCH`
- `ELIOT_SCOPE_MAPPING_FAILED`
- `ELIOT_VIEW_FENCE_MISMATCH`
- `ELIOT_RECIPE_MAPPING_UNSUPPORTED`
- `ELIOT_RESULT_MAPPING_FAILED`
- `PROVIDER_COVERAGE_DEGRADED`
- `REVERSE_WRITE_CHANNEL_DETECTED`

## Required tests

- disabled by default and every activation prerequisite;
- exact WorkScope/disclosure never-widens mapping;
- hidden membership not mappable or disclosed;
- SourceView/WorkspaceViewRevision/StateFence exact fixture and drift rejection;
- exact eleven generic recipes; no new authority surface;
- result preserves coverage/gaps/reasons/ambiguity and has no memory disposition;
- provider failure narrows coverage without unrelated-work failure;
- revocation maps to notice, not client evidence deletion;
- no canonical credential/store/reverse-write dependency;
- generic Search API digest unchanged by enabling adapter.
