# Working on search-index-reclaimer

Read the root [AGENTS.md](../../../AGENTS.md), current
[architecture](../../../docs/architecture/README.md), and
[implementation](../../../docs/implementation/README.md). Change publication,
pin admission and the actual Qdrant caller together when reclaim behavior requires it.

Take retired IDs from the committed publication owner and preserve their route.
Delete after the last relevant guard ends. A caller-created proof or an earlier
pin snapshot cannot substitute for these facts.

Use one exact delete/readback path through the existing Qdrant adapter. Carry the
real maintenance deadline and cancellation into bounded batches; resolve an
uncertain delete by reading those same IDs. Avoid another port hierarchy or an
extra transport round trip to re-establish a result just obtained.

Keep the retirement list and compact unfinished progress. Remove duplicated ID
arrays, completed receipt history and custom plan hashing when they add no retry
behavior. Ordinary reclaim does not change published visibility, acquire query
pins, delete source storage or acknowledge security purge.

Verify an actual held query guard, retirement/admission races, partial delete,
timeout/readback and restart with unfinished work. A fixture that supplies
matching proof fields does not establish committed retirement.
