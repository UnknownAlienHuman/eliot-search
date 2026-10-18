# search-retrieval-executor — implemented functions

## Current API

These are source entrypoints, not evidence that the public daemon connects them.

| Entry point | Actual behavior |
|---|---|
| `admit(plan, owner_id, lane, now_tick, scheduler)` | Checks supplied queue counts and plan limits; creates `ExecutionGuard` with a deadline. No queue reservation. |
| `schedule_leg(guard, leg, dependencies, request_security, live_security, now_tick)` | Checks dependencies, access, deadline and cancellation; clones the leg into a ticket. |
| `acquire_leg_pins(ticket, registry, now_ms)` | Returns a separate `LegPinSet` owning the acquired epoch guards. |
| `dispatch_leg(ticket, backend)` | Calls `LegBackend::dispatch`, then checks nomination shape and limits. |
| `normalize_within_population(output)` | Sorts raw scores inside one population and assigns ranks. |
| `fuse_safe_legs(outputs, profile)` | Builds a map keyed by `candidate_id`, adds weighted reciprocal ranks, sorts and truncates. |
| `discard_contaminated(decision, execution)`, `classify_completion(execution, plan)` | Discard affected leg outputs and summarize supplied execution state. No redispatch. |
| `cancel(guard, execution)` | Sets cancellation flags and clears stored outputs. |

See [lib.rs](src/lib.rs). [indexed.rs](src/indexed.rs) adds `IndexedRetrievalPort::query_filtered`,
`count_exact` and `execute_indexed_leg`: it checks the supplied route/pin/population, queries, counts
and maps nominations. The [daemon adapter](../../../bins/eliot-searchd/src/query_composition.rs) currently
uses a process-test port.

## Changes required in the connected query

- Fix fusion identity at its producer and consumer. `execute_indexed_leg` generates
  `candidate:{leg_id}:{point_id}`; two legs finding the same published point therefore do not merge in
  `fuse_safe_legs`. Resolve a result identity from the real published point/unit, source scope and
  revision. Do not merge equal point strings from different routes or incompatible identities.
  Keep leg contributions separately and pass final order to the projector.
- Make the request own its pins and cancellation through dispatch/readback. Existing `cancel` neither
  drops a separately held `LegPinSet` nor invalidates an already issued ticket; `dispatch_leg` accepts
  that ticket without a deadline/cancellation check. Setting the guard flag alone cannot stop its I/O.
- Bound combined input work before building the full fusion map. An output truncation after sorting
  every nomination does not constrain input memory. Keep only real retrieval legs and their useful
  ranking data.

The [implementation guide](../../../docs/implementation/README.md#direct-planning-and-execution)
owns real-port connection, removal of mandatory counts and request-proof inputs. Change output types
and actual callers with those removals; ordinary top-k has no completeness denominator.
