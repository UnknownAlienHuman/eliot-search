# Working on search-publication

Read the root [AGENTS.md](../../../AGENTS.md), current
[architecture](../../../docs/architecture/README.md), and
[implementation](../../../docs/implementation/README.md). Keep the product purpose
in context while changing this package and its actual consumers together.

Own the membership change and its publication lifecycle. Durable control belongs
to `search-control-redb`; Qdrant calls belong to the existing bridge; daemon
composition connects them. Remove a duplicate owner or receipt layer when a direct
call expresses the same work.

Keep complete membership, coherent visibility and recovery of unresolved external
effects. Make preparation disposable before persistence or dispatch. Work which may
have changed Qdrant must remain resolvable after cancellation or restart.

Use the planner's data once, borrow ordered manifests, and process bounded batches.
Do not preserve an old digest format, phase enumeration, unused reservation or
full receipt history merely because a fixture expects it. Retired-point deletion
belongs to the query-view lifecycle, after the final relevant guard.

Verify the changed behavior through real consumers: cancelled preparation, partial
index writes, the committed query view and restart with an unfinished publication.
Package-only observation fixtures do not establish those runtime properties.
