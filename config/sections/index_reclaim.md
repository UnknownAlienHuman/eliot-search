# Index cleanup settings disposition

The generic `search-index-reclaimer` platform and `[index_reclaim]` packet are removal targets. The current
daemon does not register this section.

Publication owns finite `RetiredCleanup` work: exact point IDs or one old generation, a cursor, route/generation
and the sequence threshold. Cleanup begins only after relevant old view guards drain; unknown deletion keeps
the same cursor and retries readback. Backpressure pauses new publication before another external mutation.

Batch limits may be private publication settings when measured. They require no independent reclaimer owner,
epoch-pin registry, section digest or live-apply receipt.
