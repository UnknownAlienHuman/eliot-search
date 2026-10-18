# Audit evidence

Start implementation with the [architecture](../architecture/README.md) and
[implementation guide](../implementation/README.md). Audits explain findings; they do not add product
requirements or an independent task queue.

| Need | Record |
|---|---|
| Actual source defects and removal boundaries | [Verified source audit](ARCHITECTURAL_AND_CODE_AUDIT_2026-10-17.md) |
| Existing libraries and inspected donor code | [Donor index](DONOR_VERIFICATION_REGISTER_2026-10-09.md) |
| Historical indexed implementations | [Donor disposition](INDEXED_SPINE_PORT_MAP_2026-10-09.md) |
| Current source work | Existing handoff and [#352](https://github.com/UnknownAlienHuman/eliot-search/issues/352) |

The [older master audit](ELIOT_SEARCH_MASTER_AUDIT_2026-10-09.md),
[completion audit](PROJECT_COMPLETION_AUDIT_2026-10-09.md), launch records and wave packets are dated
evidence. Their “next”, BLOCKED, accepted-digest and qualification-prerequisite statements do not govern
current implementation. No requirement survives only because an audit or old fixture mentions it.

Keep a source finding tied to its actual caller and observed base. Distinguish a library/type from a
connected runtime path. Test actual behavior after integration; no receipt, issue state or structural
validator can stand in for a real indexing/query result.
