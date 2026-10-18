# Working on search-projection-planner

Read the root [AGENTS.md](../../../AGENTS.md), current
[architecture](../../../docs/architecture/README.md), and
[implementation](../../../docs/implementation/README.md). Change the prepared-data,
publication and bridge consumers together where their data flow requires it.

Keep exact membership coverage, the source/unit coordinates used by retrieval,
actual vector products and coherent indexed profiles. Use the existing shared
payload/identity implementation while removing duplicate bridge representations.
A field table, byte format or separate identity crate is not permanent architecture.

Produce bounded pieces from one checked representation. Preserve the complete
membership in stored ordered data without requiring the full manifest, a generic
canonical tree and several cloned diffs in memory at once. A larger global limit
does not provide streaming or cancellation.

Borrow the same prepared vectors for upload. Derive the change set by walking
ordered entries; retain unchanged points and give changed immutable content a new
physical ID. Keep Qdrant I/O with the concrete adapter.

Verify real preparation-to-upload data flow, complete membership across batch
boundaries, incremental retention/replacement and cancellation between bounded
pieces. Historical codec equality and source-line quotas do not constrain this work.
