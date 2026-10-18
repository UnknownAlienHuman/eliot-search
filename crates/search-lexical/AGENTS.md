# Working on search-lexical

Read the [architecture](../../docs/architecture/README.md),
[implementation](../../docs/implementation/README.md) and this package's
[source map](FUNCTIONS.md). Historical frozen profiles, qualification receipts, fixture bytes and
line quotas do not constrain the replacement.

The product needs useful code term preparation. Native Qdrant BM25 handles ordinary text tokenization,
sparse encoding, weighting and retrieval. Keep the existing bounded identifier expansion,
normalization and original-span behavior where it improves code search; remove replaced sparse engines,
statistics builders and profile/fixture identity machinery with their consumers.

Use the existing `unicode-ident` and `unicode-normalization` routines for retained code behavior.
The bridge owns the existing Qdrant SDK and native `Document` inputs. Do not add a second backend,
model runtime or generic lexical-provider interface. Select matching ingestion/query options explicitly;
English stemming and stopwords are not defaults for code or neutral text.

Trace preparation, planner and real bridge/query callers before changing an API. A missing text-input
path is integration work, not a reason to invent a qualified profile or preserve a byte-compatible
encoder. Coordinate shared files with their actual writer; no contract-change issue is a prerequisite
for an already authorized coherent change.

Keep source bytes and coordinates independent of normalized search terms. Exact/source owners perform
literal and complete negative checks. Bound input, expansion and batches; propagate cancellation or
unsupported input without advertising partial work as a complete result.

Verify useful token/range behavior and connected native search with the relevant consumers. Remove
checks whose sole purpose is to preserve old sparse bytes or qualify an obsolete protocol. A changed
encoding needs consistent re-indexed content, not a universal migration or attestation platform.
