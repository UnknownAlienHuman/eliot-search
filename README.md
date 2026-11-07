# ELIOT Search

Local search for codebases and documents: ranked locations, retained-source excerpts and explicit gaps.
One OS account, plain local storage, same-host Qdrant and an owned CLI/daemon connection.
No application encryption, key management, pairing or multi-user access platform.

**Status: explicit file/tree index → Qdrant publication → verified retained search is connected.**
Search returns full retained units and citations after verifying the published snapshot's source and prepared
objects. Publication returns generation state; user queries use `search`. U1 source review and targeted Windows
debug native qualification passed at c843171; earlier transport/budget evidence has its own scope in the work
register. The full product and installed baseline remain open.

| Need | Start here |
|---|---|
| Product and boundaries | [Architecture](docs/architecture/README.md), [local scope](docs/architecture/LOCAL_SCOPE.md) |
| Implement | [Work register](docs/audit/ANALYSIS_REGISTRY.md#work-register) → [delivery sequence](docs/implementation/README.md#delivery-sequence) |
| Cleanup decisions for every package | [Workspace map](docs/audit/WORKSPACE_MEMBER_DISPOSITION_2026-10-21.md#package-register) |
| Find code and check a change | [Code map](docs/implementation/CODE_MAP.md), [working guide](docs/implementation/WORKING_GUIDE.md) |
| Find a specific rule | [Documentation index](docs/README.md) |
| Run | [Quickstart](QUICKSTART.md), with exact qualification scopes in the work register |

Use `index PATH` on an initialized root to admit and refresh that file/tree, then `index` in a new process to
reload all stored selections, followed by `search`. Explicit PATH refreshes only that scope; bare index uses
the deduplicated union. R1 remains partial: the next foreground periodic-refresh consumer, lifetime and interval
are not implemented. Search stays read-only.
Confirmed deletion excludes a source from new generations while preserving retained objects and pinned views.
Do not build optional systems first or restore the retired source/qualification frameworks.

## License

See [LICENSE](LICENSE).
