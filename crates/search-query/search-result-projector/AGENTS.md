# Working on search-result-projector

Start with the [architecture](../../../docs/architecture/README.md) and
[query implementation](../../../docs/implementation/README.md#4-simplify-and-connect-the-query).
Read the actual APIs in [FUNCTIONS.md](FUNCTIONS.md) and their producer/encoder callers.

- Project one ordered source-backed result. Preserve fused rank; raw scores from different
  populations cannot reorder it.
- Derive source/revision, coordinates and excerpt from verified data. A caller-supplied receipt,
  assurance label or unrelated handle is not that connection.
- Bound the page, snippet reads, encoded response and gap details. Select before unnecessary reads
  or handle minting; do not retain full source buffers as projection bookkeeping.
- Keep pure projection free of mutable lifecycle state. Paging and any selected opaque locators share
  the [one temporary owner](../search-continuation/README.md#one-temporary-owner).
- Change the result model, validator handoff, encoder and real client together. Remove obsolete
  mandatory envelopes, recipes and fixtures when their consumers are replaced.

Check ordering across different score scales, exact source coordinates and truncation in the actual
response. No source-shape tests or fixed recipe count define this package.
