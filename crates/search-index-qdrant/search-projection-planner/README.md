# search-projection-planner

**C13 — pure deterministic Qdrant projection planning.**

**Status:** a substantive legacy planner exists in active `main`, including bounded point/vector planning
and manifest diff. Its payload/identity contract still uses the obsolete membership/digest field set and
is not compatible with the canonical S9.5/S11 collection generation.

Draft PR #209 contains the aligned typed payload and immutable-manifest replacement. Until the stacked
Qdrant integration is accepted, this package is `LEGACY` for production publication rather than
`unimplemented` or `enabled`.

## Owns

- projection/profile/input validation over immutable prepared units;
- exactly one source membership per projection membership;
- exact point and named-vector plan construction;
- canonical payload construction;
- source-membership-to-projection mapping in control/manifests, not Qdrant payload;
- immutable exact-ID manifests and create/retain/retire diff;
- expected payload/vector identities for publication readback;
- required collection vector and payload-index contract checks.

## Must not own

- Qdrant transport or mutation execution;
- source truth, access authority or corpus management;
- broad closure filters when exact point IDs exist;
- sharing one retrieval point across memberships;
- caller-supplied authoritative digests that the planner can derive;
- parser/enricher/encoder implementation internals.

## Integration rule

The canonical planner must be integrated with:

- S11 point identity from #207;
- S9.5/S10.3 bridge from #200;
- typed publication manifests from #210;
- daemon integration from #211 and downstream query composition.

Mixing legacy and aligned manifests in one collection generation is forbidden. The integration creates a
new generation and rebuilds from immutable source/preparation truth.

- **Product area:** Architecture S8/S9/S11/S13
- **Agent instructions:** [AGENTS.md](AGENTS.md)
- **Function contract:** [FUNCTIONS.md](FUNCTIONS.md)
- **Current status matrix:** [../../../docs/product/IMPLEMENTATION_STATUS.md](../../../docs/product/IMPLEMENTATION_STATUS.md)
