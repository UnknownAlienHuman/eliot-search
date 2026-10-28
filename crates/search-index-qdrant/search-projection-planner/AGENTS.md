# Working on projection

Read [FUNCTIONS.md](FUNCTIONS.md) and the affected [publication recipe](../search-publication/FUNCTIONS.md).

Stage 2/3A builds complete bounded pages with actual retrieval text, evidence bindings and generation-bound IDs.
Do not require old/new matching, stable cross-generation IDs, sequence or closure. Query-time k/b never changes
projection identity or causes a rebuild; the lexical owner separates physical and query profiles.

Trace current callers before deleting the old proof/whole-plan APIs and move surviving consumers together.
The planner owns no effect or durable state. Inline a small assembler rather than preserve a forwarding crate.
Only measured Stage 3B work introduces source-local diff/occurrence matching.

After production changes run minimal locked Rust 1.98 check/Clippy for actual changed consumers. Digest equality
and package fixtures do not prove uploadable data or a connected publisher.
