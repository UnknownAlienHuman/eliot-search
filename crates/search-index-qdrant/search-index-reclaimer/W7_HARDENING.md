# Historical W7 reclaim/purge split

The previous W7 packet surrounded ordinary retired-point deletion with publication proofs, purge fences,
purge receipts, restore quarantine and receipt-type separation. Those products are outside baseline Search
and must not keep the reclaimer package alive.

Ordinary cleanup now has one meaning:

```text
committed publication created a RetiredCleanup row
→ all relevant live PublishedView guards drained
→ exact Qdrant ID/collection delete
→ exact absence readback
→ remove the unfinished row
```

It accepts no purge command, legal hold, backup disposition, restore manifest or broad delete filter and
makes no security-purge/secure-erasure claim. If a future regulated purge product exists, it has its own
explicit owner and may observe that ordinary bytes are already absent; it does not reintroduce a universal
lifecycle protocol into local search.

Remove W7 purge/restore/receipt interactions and tests with the reclaimer. Current cleanup and failure rules
are in [FUNCTIONS.md](FUNCTIONS.md).
