# Function contract — `eliot-search`

**Status:** thin standalone generic-client contract; no implementation exists yet.

The CLI owns argument parsing, protocol session, rendering and exit status. It never opens redb, Search
CAS, Qdrant or the OS secret store.

## `parse_command`

```text
parse_command(argv, environment, limits) -> Result<CliCommand, CliError>
```

Parses one closed command family:

- capability/status;
- pair/unpair/list-bindings where server policy permits;
- one of the exact eleven P00 recipes;
- cancel a live request;
- expand a source/continuation handle through `expand_handle@1`;
- bounded doctor request exposed by accepted server capability.

Unknown/unversioned recipes, raw vendor filters, collection/point IDs and direct store paths fail.
Sensitive values cannot be supplied as plaintext command arguments where the protocol requires opaque
proof/ref handling.

## `connect_and_authenticate`

```text
connect_and_authenticate(endpoint, local_identity, pairing_input, protocol_range, deadline)
  -> Result<CliSession, CliError>
```

Connects to the per-installation local provider transport, completes mutual hello/pairing/binding and
stores only bounded session state. Pipe ACL/local user alone is not treated as authentication.

The CLI never reads daemon credentials or constructs a Search grant.

## `fetch_capabilities`

```text
fetch_capabilities(session, refresh_policy, deadline)
  -> Result<SearchProviderCapabilityDescriptor, CliError>
```

Returns the binding-filtered descriptor. Cached data is planning/UI state only and does not bypass
per-request validation. Hidden memberships/names/counts are not inferred or displayed.

## `build_recipe_request`

```text
build_recipe_request(command, descriptor, explicit_scope, source_view, budget_class)
  -> Result<SearchRecipeRequest, CliError>
```

Builds the exact P00 tagged request. It cannot mint/sign a grant, widen scope, invent a recipe or encode
Qdrant fields. Unsupported capability returns a typed local error before sending.

## `request_standalone_grant`

```text
request_standalone_grant(session, requested_scope, requested_ceiling, deadline)
  -> Result<SearchReadGrantClaims, CliError>
```

Sends a bounded grant request to the daemon. The daemon intersects authoritative policy and mints the
grant. CLI treats the grant as opaque bounded claims and never edits memberships/ceilings/signature.

## `run_request`

```text
run_request(session, grant, request, renderer, cancel_source)
  -> Result<CliTerminalOutcome, CliError>
```

Validates request/descriptor consistency, sends one protocol request, enforces progress ordering and
accepts exactly one terminal result/error/cancelled event. Disconnect/cancel releases local resources;
server mutation uncertainty remains a typed server outcome.

## `cancel_request`

```text
cancel_request(session, request_id, deadline) -> Result<CancelOutcome, CliError>
```

Idempotent. Acknowledges `cancelled` only according to protocol semantics and never claims rollback of a
possibly committed mutation.

## `expand_handle`

```text
expand_handle(session, grant, handle, expansion, max_bytes, output)
  -> Result<CliTerminalOutcome, CliError>
```

Builds `expand_handle@1` and uses normal request lifecycle. CLI does not decode token contents, cache an
authorization permit or silently refresh an expired continuation.

## `render_result`

```text
render_result(result, mode, disclosure) -> Result<RenderedOutput, CliError>
```

Human and JSON modes preserve request/plan IDs, material ambiguity, coverage denominator kind,
freshness, assurance, degraded reasons, gaps and continuation/handle expiry metadata.

Human formatting may summarize but cannot hide a material gap or relabel `candidate_scope`/
`unknown` as `complete_scope`. JSON is canonical provider result plus bounded CLI metadata.

## `map_exit_status`

```text
map_exit_status(outcome) -> CliExitStatus
```

Closed statuses distinguish:

```text
SUCCESS
PARTIAL_OR_DEGRADED
USAGE_ERROR
AUTHENTICATION_OR_BINDING_FAILED
UNAUTHORIZED_OR_SCOPE_EMPTY
PROTOCOL_ERROR
DEPENDENCY_UNAVAILABLE
REQUEST_CANCELLED
REQUEST_FAILED
SNAPSHOT_EXPIRED
```

Material partial/degraded outcomes do not map to success unless a specific command contract explicitly
classifies the outcome as successful-with-warning and preserves a non-zero machine status field.

## `close_session`

```text
close_session(session, reason) -> CliDisconnectReceipt
```

Stops new requests, sends bounded cancellations where possible and closes local transport. It owns no
durable handle deletion or daemon shutdown unless an explicit authenticated server command is invoked.

## Configuration semantics

Presentation/progress fields may apply locally. Direct store access and partial-success exit-zero are
locked false. Standalone grant TTL is a daemon policy request ceiling, not client authority.

## Typed failures

- `CLI_USAGE_ERROR`
- `CLI_COMMAND_NOT_SUPPORTED`
- `DAEMON_UNAVAILABLE`
- `PAIRING_REQUIRED`
- `BINDING_REVOKED`
- `PROTOCOL_VERSION_MISMATCH`
- `CAPABILITY_NOT_AVAILABLE`
- `REQUEST_PARTIAL_OR_DEGRADED`
- `REQUEST_CANCELLED`
- `SNAPSHOT_EXPIRED`
- `CLI_OUTPUT_LIMIT_EXCEEDED`

## Required tests

- dependency guard: no redb/CAS/Qdrant/secret-store dependency;
- exact eleven-recipe and command closure;
- raw vendor filter/store path rejected;
- mutual pairing beyond ACL;
- CLI cannot mint/edit grant or widen scope;
- progress/terminal sequencing and cancel/disconnect;
- partial/degraded/ambiguity/coverage rendering and exit statuses;
- handle token remains opaque and expansion uses recipe;
- secret/pairing/handle/query/source redaction;
- managed/standalone owner conflict displayed without bypass.
