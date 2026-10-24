# Working on subject resolution

Read the [architecture](../../../docs/architecture/README.md),
[implementation](../../../docs/implementation/README.md) and [local README](README.md).
Resolve the user's selected entity to actual source locations and expose material ambiguity.

The current functions consume supplied candidates; their callers are tests. Connect real candidate
producers and source validation together. Keep selection as a small part of the existing query owner;
change its API and consumers when a fixed ladder or package split adds no useful behavior.

Prefer an explicit source/revision/range, then available symbol occurrences in the selected published
snapshot. Use lexical/name matches to nominate alternatives. Preserve repository, document and
configuration distinctions; identical names or supplied equivalence receipts do not establish identity.
A truncated candidate set cannot prove that its sole visible match is unique.

Use the existing manifest and retained-byte lookup. Missing, stale or ambiguous explicit locations
must remain visible; do not silently substitute a different subject. Apply the actual selected scope
and client restrictions where data is read and returned.

Remove unused resolution receipts, duplicate candidate copies and separate revalidation state with
those consumers. Reuse the ordinary result and its existing window when expansion is needed.
The [function map](FUNCTIONS.md) records current APIs; it does not freeze their signatures or require
an additional resolver framework.
