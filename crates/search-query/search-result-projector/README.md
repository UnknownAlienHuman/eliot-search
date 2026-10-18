# search-result-projector — formatting candidate for inlining

After evidence resolution, projection should only turn already ordered `VerifiedEvidenceItem` values into
bounded `EvidenceCard` values and truthful aggregate gap counts.

```text
VerifiedEvidenceItem
  final rank / contributions
  source / revision / representation / logical occurrence / exact range
  verified excerpt
  freshness / assurance derived from real records
→ EvidenceCard + optional WindowItemRef
```

It must not read source bytes, sort raw backend scores, mint handles, own continuations, issue receipts or
reconstruct authorization. `QueryWindow` owns paging/item locators; provider framing owns encoded output.

Apply card count and actual encoded-output limits before delivery. Preserve final order. Summarize omitted
gap categories with counts instead of allocating one record for every rejected candidate.

The current package's caller-assembled metadata/envelope and handle binding are removal targets. If the
remaining transformation is a small loop, inline it into query composition and delete this crate rather
than retain another package boundary.

[FUNCTIONS.md](FUNCTIONS.md) records the existing defects and replacement.
