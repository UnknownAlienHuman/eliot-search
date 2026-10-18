# Working on sources

Read the [architecture](../../docs/architecture/README.md) and
[implementation](../../docs/implementation/README.md). Complete the path from a selected root to
retained bytes and a committed source head. Change its actual producers and consumers together;
preserve #598's active manager/private work.

A path locates a source. Evidence refers to an exact retained revision. Watcher events request work;
the read/reconciliation result determines current state. Transfer the bytes actually acquired to the
writer instead of rereading a pathname after its identity was observed.

Use one source/revision model and one immutable-byte I/O implementation. Pure admission decisions
need no permanent receipt issuer, custody chain or separate lifecycle package. Remove obsolete v1/v2
converters and duplicated records with their consumers. Keep meaningful scope and stable-read checks.

These directories show the current layout. Consolidate packages that only split one responsibility;
agent assignments and historical contracts do not define permanent dependency boundaries.
