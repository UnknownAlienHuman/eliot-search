# Working on search-result-projector

Read the [architecture](../../../docs/architecture/README.md), current
[implementation](../../../docs/implementation/README.md) and [query cutover](../README.md).

Keep this package pure and small:

- preserve the final ranked order supplied by retrieval/fusion;
- accept only selected, source-verified evidence items;
- format bounded cards, excerpts and truthful gap counts;
- enforce encoded output limits as well as card count;
- use an optional `WindowItemRef`, not a separately minted source handle.

Do not read source bytes, mint handles, manage continuations, sort by raw score, own coverage proofs or
create result fingerprints. Change result contracts, encoder and real caller together. If the final
transformation is trivial after that cutover, inline it into query composition rather than preserving a
crate for its fixtures.
