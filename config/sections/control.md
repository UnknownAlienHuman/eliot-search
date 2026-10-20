# Control settings disposition

The current registry exposes `durability` and `migration_epoch`, but the generic snapshot does not construct a
small current-state redb owner from them. The older timeout/idempotency/snapshot/migration-policy catalogue is
not an implemented file API.

The target has no public generic `[control]` section initially. The current-format owner selects one redb
format/path/durability profile directly and performs short typed transactions. Pre-release schema migration,
quarantine policy, operation archives, receipt TTLs and full snapshot publication are removed.

Add a field only when the surviving redb owner consumes it. A static format/durability change is a new current
format or restart decision, not a live config receipt.
