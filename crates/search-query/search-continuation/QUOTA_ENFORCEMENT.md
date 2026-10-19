# Query-window quota enforcement

Use ordinary checked counters owned by the window manager and current request. Quotas are not authority
records and do not need receipts.

Before creating a window, reserve its result-reference count and estimated metadata bytes against aggregate
manager capacity. Before each page/expansion, reserve item count, retained-source read bytes and encoded
output bytes against both the window and request context. Charge actual bounded values as they are retained.

If a limit cannot be satisfied:

- before window creation: return a resource limit and retain nothing;
- before page evidence reads: return an explicit incomplete/resource gap and do not advance;
- while formatting: stop at the last complete item which fits the selected response contract, report omitted
  counts and commit only the emitted complete page;
- on impossible metadata/corruption arithmetic: fail the window.

Do not scan every live record to derive authority, collect unbounded invalidation batches or keep terminal
quota history. Expiry/removal releases aggregate counts through ordinary ownership/drop.
