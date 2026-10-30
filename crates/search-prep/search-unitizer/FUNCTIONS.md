# Unitization — one prepared product, generation-bound baseline

This owner selects bounded units and truthful representation ranges. It owns neither source currentness nor
backend point lifetime. [Materializer](../search-materializer/FUNCTIONS.md) owns preparation input and installation;
[lexical](../../search-lexical/FUNCTIONS.md#retrieval-text-no-alias-inflation) owns retrieval-text meaning.

## Baseline output

Start with line-preferred ranges over unchanged UTF-8. Preserve byte/character boundaries and complete raw coverage
without a per-character identity map. Split oversized ranges locally; no syntax tree or overlapping structural
model is required. Other selected formats use their actual structures/maps, splitting oversized units or merging
small peers only within the same context. Preserve gaps and coordinate domains.

A unit needs its kind, representation range and only the selected context. Source/input/transform information
shared by every unit belongs once on the enclosing product; it need not be copied into every descriptor.

## Representation and ownership

Keep one owned text buffer with bounded range/context descriptors, not one owned copy of source text per unit.
For a mapped format, retain only the actual coordinate/loss data its evidence reader uses. Raw coverage and
structural overlap are different facts; do not sum overlapping ranges into complete-source coverage.

Encode the representation and descriptors once through the shared object writer. The prepared artifact cannot
contain its own content-addressed ObjectRef: that reference is returned after publication of its bytes. Bind it
from the current preparation row and later evidence records, not a placeholder, cyclic hash or follow-up rewrite.
Preserve the retained input reference and transform binding required to validate the product.

The [projection assembler](../../search-index-qdrant/search-projection-planner/FUNCTIONS.md#stage-23a-assembler)
materializes actual retrieval_text and context into bounded upload pages from those ranges. Do not retain both a
full document and all copied per-unit strings merely to pass between adjacent stages. Charge the temporary page,
serialization expansion and total prepared output; a count-only unit limit is not a memory bound.
Once publication captures pending pages, their actual text/payload stays fixed for retry. Do not regenerate them
from a newer source, changed options or a fresh parser run. No digest-only upload or second text composer.

## Stage 2 / 3A — no occurrence matching prerequisite

Publication derives generation-bound backend IDs from captured source/preparation/unit inputs. Unitization does
not mint Qdrant IDs or store physical collection names in a reusable prepared artifact. A unit's bounded ordinal
inside that exact immutable artifact is an address, not evidence of continuity with another revision.

A changed source may remint every point in the next fresh generation. An unchanged retained input with identical
output-affecting settings may reuse its complete preparation without old/new matching. The pinned evidence
snapshot supplies exact source/prepared/range truth; Stage 3A measurements determine whether rewriting is acceptable.

## Stage 3B — conservative reuse only when budgets require it

Only after the delivery guide selects measured incrementality: compare bounded old/new descriptors within the
same source/context, including actual retrieval input. Reuse only unambiguous one-old/one-new matches. Equal
duplicates, ambiguous parents and unmatched units get operation-fixed births in the pending delta; disappear/
reappear never reopens a closed interval. Ordinal, equal text, name or range shift alone is not continuity.
No general AST diff, LCS or lineage service before demonstrated need.

## Empty and gap semantics

Supported complete empty input or complete text with no searchable units yields zero units and retains source
binding. Unsupported, truncated, cancelled or unavailable extraction is a gap/incomplete outcome. Partial mapped
text keeps its actual coverage; no sentinel unit/vector or complete-looking partial source.
Nonempty→empty changes membership through publication, not direct point deletion here.

## Removal

Remove old revision/coordinate-derived stable-ID claims, nested v3/v4/v5 manifests, adjacent parser replay,
discarded canonical encodings, duplicate writers and baseline retain/create/close prerequisites with real callers.
Keep useful layout and range checks; replacing internal formats does not authorize deleting original user data.

## Verification

After code run minimal locked check/Clippy for actual preparation/publication/evidence consumers. Later baseline
cases: exact UTF-8 coverage/boundaries, mapped ranges, oversized unit/output, bounded live buffers/pages, supported
empty, saved-product reuse, changed-source remint and old-view evidence. Stage 3B adds ambiguity, fixed-birth replay
and measured write-reduction cases only when selected. This recipe supplies no runtime result.
