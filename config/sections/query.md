# Query settings

Read the [implementation guide](../../docs/implementation/README.md) and
[configuration change map](../../crates/search-config/FUNCTIONS.md). This packet distinguishes actual
registered values from the former proposed query-settings catalogue.

## Current daemon setting

[registry::query_section](../../bins/eliot-searchd/src/config_composition/kernel/registry.rs) currently
registers only `query.limit`: integer, default 10, allowed from file/environment/CLI, `ApplyLive`.
The current startup compares it with defaults and refuses a changed value without `LiveApply` in a
supplied receipt-kind set. That is the implementation to replace, not an acceptance requirement.

## Target

Use a typed positive finite query limit as the default for new requests. Validate its range, pass it
to the real handler and keep the request's admitted limits stable while it runs. Capture and merge
settings once at startup; no receipt is required before constructing the handler with them.

Candidate, readback, queue and output limits belong to the actual query path. Document configurable
fields only when they have a real consumer. The previous `default_budget_class`, scoring-leg/prefetch/
validated-candidate catalogue and `fusion_profile_id` were not the fields in this daemon registry.
Do not make another settings registry to preserve that old table.

Keep the published lexical encoding profile coherent with indexed vectors. A request/default change
is not automatically a projection rebuild; an actual indexed profile change uses publication.
No section digest, granted budget class, qualification receipt or wave permission is needed for
ordinary local query settings.
