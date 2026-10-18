# search-query-planner

Pure query planning inside the application: choose the operations needed for a resolved source scope,
published view, retrieval profile and finite query limits. Storage, scope resolution, Qdrant calls and
source readback belong to their existing owners.

Use the [implementation guide](../../../docs/implementation/README.md#4-simplify-and-connect-the-query)
for the integration change and [FUNCTIONS.md](FUNCTIONS.md) for the actual API.

## What exists

[src/lib.rs](src/lib.rs) implements recipe normalization, bounded leg construction, budget allocation,
drift classification and canonical dependency/fingerprint checks. `compile_canonical_plan` returns
`CompiledSearchPlan`; it consumes a caller-created `SearchTaskPlan`. The inspected tree has tests and
support callers for compilation, but no connected production CLI/daemon query calling it.

The current API uses V1 recipes and grant/fingerprint inputs. The delivered evidence/orientation
contracts use V2 forms. Their existence does not connect either path to execution.

## Change this owner with its consumers

Accept resolved scope, a pinned published view, the actual lexical profile, query and limits directly.
Remove the mandatory task-plan/grant/fingerprint assembly from trusted-local planning. Keep useful
ordering, scope/profile checks, leg bounds and drift behavior; do not create synthetic proof inputs or
another planner to satisfy the old API.

Connect the daemon query handler to this owner, existing lexical encoding, retrieval and source
validation. Update request and executor consumers together. Implement selected operations as they
become usable; preserving all eleven V1 recipes and their bytes is not a prerequisite.

Planning performs no I/O or pin acquisition. The runtime owns one request deadline; the executor and
reader enforce actual work limits. An ordinary ranked query needs neither an exact denominator nor a
persistent plan. See [AGENTS.md](AGENTS.md).
