# ELIOT Search configuration

Configuration supplies values to concrete runtime owners. It does not certify capabilities, issue receipts or
own a generic reconfiguration transaction.

Read the [implementation guide](../docs/implementation/README.md),
[source/deletion map](../crates/search-config/FUNCTIONS.md),
[typed TOML replacement](../crates/search-config/FUNCTIONS.md#target-structures-and-merge) and
[source audit](../docs/config/CONFIGURATION_1.0.md).

## Current source truth

The public daemon still uses a custom schema-2 parser, dynamic registry, generic snapshot/fingerprint and
receipt-gated activation. Its registry contains only eight section families and most values do not reach typed
runtime consumers. The checked-in example previously listed many unregistered fields; it is now an explicit
legacy-parser no-op, not a product configuration.

The `sections.toml` registry and individual packets are removal/audit inputs. Some packets already describe the
new direction; others are historical and are being reduced to concrete dispositions. Neither registry nor
packet can make a field supported.

## Target

```text
capture args_os once
+ bounded TOML/Serde file
+ selected vars_os / field-specific CLI overrides
→ concrete DaemonSettings
→ validate actual values
→ construct actual owners/handlers
→ status reads retained running settings and observed component state
```

Only fields with a real consumer exist. Unknown fields/types/ranges fail before side effects. Paths stay OS
values. Optional features add settings only with their real handler.

Runtime changes are direct owner operations: query default update, source reconcile/publication, process
restart or retrieval-generation publication. There is no generic action DAG, configuration journal, completed
receipt history, fingerprint readiness or compatibility decoder.

Issue [#333](https://github.com/UnknownAlienHuman/eliot-search/issues/333) owns the cutover. Historical W5–W10
configuration files remain evidence only and are deleted with their last tooling consumers under #214/#525.
