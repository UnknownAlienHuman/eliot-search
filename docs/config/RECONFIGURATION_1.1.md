# Configuration construction and runtime changes

Configuration is data for current owners. It is not a transaction coordinator for every subsystem.
Read the [source audit](../../crates/search-config/FUNCTIONS.md) and
[typed settings recipe](../../crates/search-config/FUNCTIONS.md#target-structures-and-merge).

This is target behavior. The source audit names the current status/startup, locator and DEK-selection
consumers; ordinary query settings are not yet connected to a query owner.

## Startup

```text
capture one invocation
→ bounded file/env/CLI decode
→ merge over defaults
→ validate one DaemonSettings
→ construct actual owners/handlers
```

There is no running defaults-only application to update. Remove startup `try_activate`, fingerprint diff and
receipt gating. A valid non-default `query.limit` reaches the query owner directly; do not fabricate a
`LiveApply` receipt.

The current daemon's generic snapshot stores no typed query/lexical/control values and separately requires
`ValidatedSecretsConfig`. Replace it with concrete settings whose fields are consumed at construction.

## Factual readiness

Current `derive_readiness` accepts `AcceptedReceipts { search_accepted, indexed_accepted,
optional_gate_accepted }`. It can therefore report a live handler unavailable because a Boolean receipt is
false, or make an unconnected capability appear closer to ready because a Boolean is true.

Replacement readiness is observation of the actual objects:

```text
root/control/object owners opened?
source/preparation state available?
Qdrant connected and selected generation verified?
CurrentPublishedView present?
Search/Exact/Continue/Expand handlers constructed?
```

Optional semantic/document/managed-process features report their own unavailable state and do not disable
baseline lexical Search. Configuration fingerprint equality is not readiness proof.

## Changes to a running application

| Actual setting | Behavior |
|---|---|
| Query defaults for future requests | Validate and directly replace the query owner's current setting; admitted requests keep their captured limits. |
| Source roots/policy | Execute actual bounded reconcile/publication when implemented; otherwise require restart. |
| Data root/storage profile/Qdrant endpoint/process profile | Restart/reopen; do not attempt live migration through config. |
| Lexical/model/schema/retrieval profile | Build and publish a coherent new generation through publication. |
| Optional handler enablement | Construct/stop the real handler/profile; until implemented, reject as unsupported. |
| Unknown or unsupported change | Leave current settings unchanged and return a clear error/restart requirement. |

Only publish a new current settings value after the direct owner operation succeeds. A possible external
Qdrant mutation remains publication's pending operation. Local known abort is known failure.

Persist a small current applied settings row only if a runtime update must survive restart independently of
the startup file/environment. Do not append a config operation receipt, keep terminal history or rebuild the
entire control snapshot.

## Status

Status reads the retained running settings and component observations. Serialize a small redacted struct with
Serde. Do not:

- reread file/environment/argv;
- rebuild dynamic registry/sections;
- generate a canonical configuration fingerprint;
- list receipt/gate acceptance;
- expose secret values/private paths;
- claim readiness from configured intent rather than constructed/observed behavior.

Useful output includes selected modes/profiles, safe limits, source counts/gaps, Qdrant connection/generation,
current view and actual handler availability.

## No generic reconfiguration platform

Delete:

- `ReconfigurationAction`, topological action ranks and capability-owner sets;
- `ReconfigurationPlan` / `ReconfigurationStep`;
- `ReceiptKind` and `try_activate`;
- old/candidate config fingerprints;
- generic security-floor/restrictive-change calculations without a concrete consumer;
- config intents/effect DAG/recovery journals;
- old section/profile/schema migration.

A concrete operation may have several steps. Put them in its existing owner and use that owner's actual
pending/recovery mechanism. Do not recreate a configuration-wide coordinator.

## Focused behavior

After implementation verify through real consumers: startup non-default values, precedence, invalid input,
non-Unicode environment/path behavior, bounded file read, direct query-default update, one restart-required
change, one profile generation publication, factual readiness and status. No all-field hot-reload matrix or
receipt-gate qualification is required.
