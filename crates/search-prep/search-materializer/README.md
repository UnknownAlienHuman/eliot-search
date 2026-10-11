# search-materializer

**C08 — Materialization.**

**Status:** baseline text/source-code materialization, profile validation, coordinate/loss maps, assurance classification, and the optional-provider qualification seam are implemented. During T02 migration the package also owns the bounded legacy DIRECT preparation-object/reference filesystem lifecycle, frozen binding/reference/manifest schema, and physical-inventory filename/classification grammar. Concrete optional document-provider qualification remains gated and no provider acceptance is implied.

Convert an exact retained revision into a canonical representation with explicit coordinate and loss maps.

The additive inert Markdown API produces an immutable raw-backed product and exact-source replay
receipt. It is source preparation only: no UnitSet, document-provider acceptance or runtime/product
qualification is implied.

## Owns

- materializer profile contracts
- raw text/source-code baseline materialization
- coordinate map and loss map production
- assurance ceiling classification
- provider qualification seam for optional documents
- inert Markdown profiles, bounded original-range observations, shared canonical commitments and complete exact-revision replay verification
- bounded exact legacy preparation-object/reference reads
- no-clobber immutable preparation publication with native-identity fencing
- exact `ELSPRP02` binding and `ELSPRF01` reference layouts
- legacy manifest framing, fixed algorithm/profile tags and validation
- lookup/object digest preimages, bounds and canonical lower-case locator names
- closed `refs`/`objects` tree and final/temporary basename grammar
- stable physical/current inventory classification tags and relative locators

## Must not own

- selecting a PDF/Office/OCR provider without ADR
- authority or ranking
- executing macros, archive members or remote resources
- claiming exact coordinates after lossy transforms
- opening source stores directly instead of consuming immutable contract inputs
- revision CAS / revision-object storage
- DPAPI, keyring or secret ownership
- source-registry or control-journal mutation
- daemon data-root traversal, metadata or platform identity observation

The compatibility filesystem adapter treats bytes as opaque. It neither proves a
materialization profile nor authorizes a source; it only preserves exact bounded
artifacts under qualified platform observations. The pure legacy-store codec and
inventory grammar own persisted bytes, digest/name derivation and closed physical
classification while concrete hashing, DPAPI, filesystem traversal and catalog
overlay remain injected by daemon composition.

- **Delivery wave:** W2 baseline / P04; optional P17
- **Soft source-line target:** 7,000
- **Agent instructions:** [AGENTS.md](AGENTS.md)

## Inert Markdown source preparation

`MarkdownProfile::new` requires a validated Text-only/UTF-8-only raw profile: BOM and invalid sequences
are rejected, newlines are preserved exactly, Unicode normalization is absent and loss is rejected.
Its identity is computed internally. Pure setup has fixed 512 Ki logical work units and 16 KiB logical
storage, without an operation clock. Source and fixture definitions bind exact embedded bytes, including
line endings; authored fixture identity does not prove fixture execution.

`materialize_markdown` reuses the validated request, injected exact revision port and raw context,
with additional profile/operation limits and the original caller deadline. The private
`pulldown-cmark =0.13.1` dependency has defaults disabled and uses empty parser Options. Raw bytes and
maps remain the representation; HTML and destinations remain inert. Read-only records expose checked
whole-event spans and selected mapped code/info/destination/title values. Ordinary decoded Text/HTML
strings are not retained as another representation. Overlapping observations are not additive coverage.
`ObservedSpansAndSelectedValues` and `PotentiallyIncomplete` reference metadata remain explicit:
reference fallback can leave links as ordinary text, so references are not claimed exhaustive.

`verify_markdown` reopens the same exact retained revision through the same preparation path and
compares the complete raw/maps/profile, ordered inventories, selected values, metadata status, usage
and commitments. Root coverage or stored digest equality alone is insufficient. Refusal returns no
complete product or replay receipt.

Original cancellation/deadline checkpoints bracket donor calls and terminal results. Every next
attempt, including terminal None, is charged before advancing; yielded events are counted separately.
Logical storage accounts owner records/frames/selected strings, residency copies and row temporaries,
including original inventory held during replay. It excludes borrowed raw bytes, existing raw/maps,
allocator spare capacity and donor internals. Logical work is not measured CPU work, and synchronous
donor calls have no hard-preemption or donor-memory guarantee.

The separately versioned Markdown structural UnitSet is delivered in search-unitizer; JATS remains under
[#226](https://github.com/UnknownAlienHuman/eliot-search/issues/226). Durable persistence/caller adoption
belongs to #331. All new fixtures are AUTHORED_NOT_RUN / NOT_COMPILED; execution and document/native/
resource/product qualification remain #215/#216.

## Inert JATS source preparation

The additive JATS API accepts a strict UTF-8 Journal Publishing 1.4 subset through private
quick-xml 0.42.0 borrowed events with every optional feature disabled. The profile binds its exact
donor, parser options, finite limits, closed syntax/namespace policies and source/fixture definitions.
It requires the same exact raw Text/UTF-8 profile as Markdown. DTD/DOCTYPE, processing instructions,
custom entities, unknown elements/attributes/namespaces, rebinding and unsupported encodings reject.
The fixed subset includes front/journal metadata, titles, body sections, lists, figures, tables,
references and inert links. Parsing this subset does not claim full JATS schema validity.

materialize_jats retains the original XML and maps. Element/comment spans refer to original bytes;
normalized text and attribute values are mapped observations with whole-event provenance. Resources
are never fetched. Complete ordered inventories use fixed 256-record canonical chunks, preserving
all records beyond the shared single-array capacity. Shared CBOR/BLAKE3 is the only identity producer.
verify_jats reopens the same revision, reconstructs the full raw/maps/profile/inventory/usage/product
and compares every field before issuing a receipt. A consistently rehashed omission still refuses.

Positive caller ceilings narrow input/events/depth/records/values, cumulative logical storage/work
and actual successful complete canonical preimages. Original cancellation and absolute deadline
survive raw preparation, donor calls and replay. Logical work/storage are not CPU/RSS guarantees;
input-bounded synchronous donor calls cannot be hard-preempted. Original raw max_steps is separate.

This #444 delivery is SOURCE_ONLY. The structural JATS UnitSet follows serially under #226;
persistence/caller adoption remains #331 and requires a versioned rebuild. Ten private fixture
functions are AUTHORED_NOT_COMPILED/NOT_RUN. Official sample, native/resource/installed and product
qualification remain #215/#216. Existing raw, Markdown and legacy profiles/APIs/bytes are unchanged.
