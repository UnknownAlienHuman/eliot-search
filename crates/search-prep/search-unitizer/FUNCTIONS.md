# Unitization — one prepared product, generation-bound baseline

Unitization turns one materialization product into bounded source-backed units. It owns structure/ranges and the
retrieval text supplied to publication. It does not own Qdrant point lifetime or source currentness.

## Baseline output

Use actual Markdown/JATS sections/items, a selected parser's real code structures, or line-preferred plain text.
Split oversized units locally and merge small peers only under the same context. Preserve exact representation and
source maps/gaps.

Each bounded descriptor contains at least:

```text
source/prepared reference
unit kind
representation/range
bounded retrieval_text
optional structural title/signature/path context selected by profile
```

`retrieval_text` contains the unit text and selected context exactly once. Do not append snake/camel/acronym aliases:
Qdrant 1.20's `code` tokenizer already splits naming conventions and indexes the joined identifier. Alias inflation
changes term frequency and document length.

Encode the prepared artifact once through the shared immutable object writer. Do not reparse adjacent stages or
embed several historical manifests.

## Stage 2 / 3A — no occurrence matching prerequisite

The walking skeleton and fresh-generation convergence do **not** need continuity between old and new units.
Publication captures a complete prepared inventory and derives deterministic generation-bound point IDs from:

```text
physical generation
source identity
bounded unit position/key inside the captured prepared outcome
retrieval profile/role
retrieval-input identity
```

A changed source may be re-unitized and all of its points reminted in the next fresh generation. Exact evidence in
the pinned redb snapshot supplies revision/range truth. This is correct even when a small edit rewrites many IDs;
Stage 3A measurements determine whether that cost is acceptable.

An unchanged retained input plus identical output-affecting preparation settings may reuse its complete prepared
artifact. That reuse does not require old/new occurrence matching.

## Stage 3B — conservative reuse only when budgets require it

If Stage 3A fails a target corpus budget and #259/#329 activate incremental publication, add source-local matching:

1. compare bounded old/new descriptors under one source-stable parent scope;
2. include actual retrieval text/profile/context in the match key;
3. reuse only an unambiguous one-old/one-new match;
4. equal duplicates, ambiguous parents and unmatched units receive operation-fixed new births;
5. disappear/reappear gets a new birth and never reopens a closed interval.

Do not infer continuity from ordinal, equal text, name or range movement. Do not build a general AST diff, LCS or
lineage service before measured benefit. The pending source delta persists any new births so recovery repeats the
same operation.

## Empty and gap semantics

- Supported complete empty input is a successful prepared outcome with zero units.
- Complete supported text with no searchable units is successful empty, not failure.
- Unsupported, truncated, cancelled or unavailable extraction is an explicit gap/incomplete outcome.
- Partial mapped text retains actual coverage/gaps and is never relabelled complete.

No sentinel unit/vector. A nonempty-to-empty change builds a valid new Stage 3A generation with no points for that
source; missing metadata/collection is not empty success.

## Removal

Remove revision/coordinate/ordinal-bound unit IDs, duplicate v3/v4/v5 manifests, adjacent-stage Markdown/JATS
verification/reparse, duplicate writers, test/profile receipt compatibility and planner APIs which require
source-local retain/create/close in the baseline. Keep parser/range logic and ordinary causal tests.

## Verification

Baseline checks: one materialization, exact ranges/maps, bounded retrieval text, no duplicate aliases, supported
zero units, changed-source remint in a fresh generation and old-view evidence stability. Stage 3B checks only if
activated: unique shift, duplicate insertion/removal/reorder, changed context, operation-fixed births and measured
write reduction. R051 adds no compiler/runtime result.
