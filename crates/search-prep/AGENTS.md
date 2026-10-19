# Working on preparation

Read the [architecture](../../docs/architecture/README.md),
[implementation guide](../../docs/implementation/README.md) and [preparation map](README.md).

Preparation turns one retained revision into one derived product and structure-first UnitSet while preserving
exact evidence coordinates. Decode/parse once and pass the owned result forward. Do not reopen/reparse inside
an adjacent in-process stage to manufacture a receipt. Validate persisted/external artifacts at their actual
acceptance boundary.

Change materializer, unitizer, prepared-artifact persistence and immediate publication/query consumers
together. Remove duplicate legacy stores, nested manifests, discarded canonical encodings and old
receipt-bound formats with their callers. Their package separation is optional; do not add a generic pipeline,
provider registry or compatibility family.

Keep transformations/profile/source maps only when a real consumer needs them. Model/tokenizer packing is a
derived projection, not source occurrence identity. Reuse outputs by their exact input/context/model/options,
not by whitespace-normalized text or positional IDs.

`search-code-enricher` is optional and disconnected. Do not add Tree-sitter/SCIP/Ctags/SARIF programmes
before a real outline/navigation consumer exists. Delete it if no such profile is selected.

After production code, run minimal Rust 1.98 compiler/Clippy for changed preparation and immediate consumers.
Focus on one retained read/parse, exact ranges, bounded memory, restart artifact readback/rebuild and no stage
leak. Historical format/golden/qualification matrices are removed with their APIs.
