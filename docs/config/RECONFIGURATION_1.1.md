# Configuration construction and runtime changes

Current implementation guidance follows the [manifesto](../architecture/README.md) and
[implementation guide](../implementation/README.md). This file replaces the old mandatory composite
obligation/receipt contract. Source details are in [search-config](../../crates/search-config/FUNCTIONS.md).

## Startup

Parse one invocation, capture selected inputs, merge supported overrides over defaults and validate
actual setting values. Construct the required components with those values. No running defaults-only
application exists at this point, so no live-apply receipt is needed to choose a non-default value.

The present daemon does the opposite: `effective_from_process` calls `try_activate` with an empty
receipt set. A changed `query.limit` is classified `ApplyLive` and refused. Remove that dependency and
connect the accepted typed setting to the handler; do not insert a synthetic `LiveApply` receipt.

## Changes to a running application

| Actual change | Implementation |
|---|---|
| Default limit for new requests | Validate and replace the owner's setting when live update is supported. An admitted request keeps its own limits. |
| Static process/connection setting | Require restart or invoke an existing supported owner restart. Report what actually happened. |
| Lexical/profile/index representation change | Build and publish a coherent generation through the existing publication owner. Its pending record covers unfinished external work. |
| Removal of a selected source from scope | Change the real source/view state; new requests resolve the new scope, and expansion respects the actual current policy. |

Do not promise a generic hot-reload service for every field. A real operation may need several effects;
execute those concrete effects without erasing one through a severity enum. That observation does not
justify a universal action DAG, config journal, gate receipts or a second cross-store recovery engine.

Keep one applied settings value with the running owner. Invalid changes leave that value unchanged.
A possible external write cannot be treated as rolled back because its acknowledgement timed out.
Reuse the existing pending publication/recovery mechanism where that effect belongs.

Status reports running settings and actual component state. It neither reparses process inputs nor
activates a newly read file. Optional components are available only when connected and working; their
future release test reports are not runtime constructor inputs.
