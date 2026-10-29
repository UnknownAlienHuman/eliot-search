# Working on optional model support

This directory is a design note, not a Cargo package. Keep it that way until one exact measured model has a
real preparation or query consumer.

Delete the always-unavailable model worker and unused `ModelProviderPort`; do not implement a generic provider
registry, plugin framework, qualification receipt, model lifecycle OS or capability flag in advance.
Baseline Search uses Qdrant native BM25 and remains independent of optional models.

A later implementation issue must name the exact model/artifact/tokenizer/task/options, measured need,
packaging, finite limits, direct consumers and failure semantics. Prefer an in-process library. Create a
worker only for demonstrated dependency/crash/resource isolation, using the same bounded local framing.

Model code may encode bounded prepared/query inputs or rerank a finite supplied candidate set. It cannot own
source discovery/scope, Qdrant publication, citations, generated answers, query rewriting, graphs or research
scheduling. Cache only by exact input/context/role/model/options; current source bindings remain with the
ordinary manifest/publication owners.

After future code, run minimal Rust 1.98 compiler/Clippy for the concrete owner and real consumers, then
measure against lexical baseline. Until then, no model code is correct.
