# Working on search-source-history

Read the [architecture](../../../docs/architecture/README.md),
[implementation](../../../docs/implementation/README.md) and [README](README.md). This package is a
transitional relocation, not an independent source owner.

Keep only revision/head values still used by the real retained-object and redb callers. Change those
callers together, then move the values to their actual owner and delete forwarding/reexport boundaries.
Do not add operation history, namespace cutover, registry replay or compatibility codecs for the new
current format.

No source read, admission, currentness, access or publication authority belongs here. Preserve #598's
active private work; do not assign a competing writer. Verify the connected retained revision/current head
across restart rather than package replay fixtures.
