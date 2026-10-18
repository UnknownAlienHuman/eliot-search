# Working on search-retrieval-executor

Start with the [architecture](../../../docs/architecture/README.md) and
[query implementation](../../../docs/implementation/README.md#4-simplify-and-connect-the-query).
Read [FUNCTIONS.md](FUNCTIONS.md), the changed source and its actual daemon callers.

Execution is part of one query operation. Change its API, planner, real adapter and result consumers
together where required; obsolete package boundaries and fixtures do not require a parallel executor.

- Keep actual scope, profile and published-view binding before retrieval.
- Own pins and one absolute deadline/cancellation context through dependent I/O; a copied ticket or
  caller-supplied flag is not ownership of the running work.
- Normalize within populations and fuse ranks across them. Deduplicate actual result identities,
  including their source/revision and route context; never use leg-prefixed IDs as result identity.
- Preserve the resulting order through validation/projection. Report failed work and limits honestly.
- Bound nominations and retained bytes before collecting the entire input. Pure ranking needs no
  durable receipts, scheduler framework or request fingerprint hierarchy.
- Use the existing real bridge at the concrete daemon seam. A process-test port proves only its test.

Check behavior that can fail: overlapping legs, score scales, saturation and cancellation with live
resources. Documentation, type construction and successful mocks do not demonstrate runtime integration.
