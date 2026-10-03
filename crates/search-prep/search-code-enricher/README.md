# search-code-enricher

**C10 — bounded no-execute Rust structural enrichment.**

**Status:** substantive Rust source exists for profile validation, tolerant syntax scanning, structural
facts, descriptive relations, configuration predicates, anchor checks and deterministic manifests. No
parser artifact/profile has been independently qualified and wired through the supported product path.

## Owns

- exact Rust parser/enrichment profile identity and qualification binding;
- bounded immutable representation input;
- tolerant no-execute syntax nodes and explicit degradation gaps;
- definition/reference/caller/test/documentation/configuration evidence roles;
- descriptive structural relations;
- `cfg`/`cfg_attr` predicate preservation;
- exact representation/unit anchors;
- deterministic enrichment manifests and profile-change classification.

## Must not own

- compiler-grade certainty from tolerant parsing;
- running Cargo, rustc, build scripts, procedural macros, language servers or shell commands;
- network access or repository code execution;
- source acquisition, durable source ownership or Qdrant transport;
- ranking, final comparison verdicts or client admission;
- vendor parser node types in public APIs.

The baseline source is useful as a deterministic structural provider, but it remains `SOURCE`, not
`QUALIFIED` or `ENABLED`. Product activation requires an exact parser package/version/checksum/license,
golden fixture digest, no-execute qualification, package check/Clippy and end-to-end projection/readback
coverage.

Structural facts are candidate/navigation evidence. They do not prove compiler resolution, runtime
behavior or semantic correctness. Unknown relation targets remain explicit and ambiguous.

- **Product area:** Architecture S17/S21, baseline code profile
- **Agent instructions:** [AGENTS.md](AGENTS.md)
- **Function contract:** [FUNCTIONS.md](FUNCTIONS.md)
