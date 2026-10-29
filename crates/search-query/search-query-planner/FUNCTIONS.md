# Query selection — direct handlers

The [query owner](../README.md) owns request/result meaning. The inherited planner is a removal target; trace
current callers before inlining/deleting it and move actual consumers in the same cut.

## Direct baseline

```text
typed request + operation context(view, scope, execution; scoring for Search) + constructed handlers
Search   → Qdrant TextQuery
Exact    → retained-source scan
Continue → stored QueryWindow page
Expand   → emitted-item bounded context
```

Optional operations appear only with actual producers/handlers. Unsupported modes fail before source/backend
work. A call to one handler requires no recipe, capability set, task/leg plan or fingerprint.

## Search selection

Use the view's physical collection/text field and the admitted request's separate query-scoring profile. Do not
read k/b from physical view identity. The [scope owner](../search-access/FUNCTIONS.md) supplies:

```text
base = selected sources ∩ real session/access restriction
candidate filter = base + request-only narrowing
IDF corpus = base
```

For an entire-collection base, omit its redundant predicate and IDF override, not request-only clauses. Omit the
candidate filter only when no clauses remain. Known empty performs no RPC. Free text nominates by TextQuery;
explicit required/excluded clauses remain filters, not exact spelling/phrase proof.

Only a real operation over incompatible collections/profiles justifies a finite private branch list and explicit
bounded rank merge. Branches share the absolute deadline/counters; raw scores from different populations cannot
be directly sorted. Do not create per-source legs in a compatible collection.

## Remove and verify

Delete V1 recipe tables, public capability/task/leg/plan records, grants, overlap/drift/dependency proofs, relative
leg timeouts and exclusive fixtures. Keep only nontrivial actual selection helpers, otherwise delete the crate.

After code run minimal locked Rust 1.98 check/Clippy for real consumers. Check direct/unsupported dispatch,
whole collection plus request-only narrowing, empty/unknown scope, candidate/IDF separation, unchanged k/b physical
identity and order preservation. Additional branch cases exist only with a real branch consumer.
