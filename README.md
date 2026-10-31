# ELIOT Search

Local search for codebases and documents: ranked locations, retained-source excerpts and explicit gaps.
One OS account, plain local storage, same-host Qdrant and an owned CLI/daemon connection.
No application encryption, key management, pairing or multi-user access platform.

**Status: the persistent product is not yet connected end to end.** Documentation describes the target;
[source refactor #610](https://github.com/UnknownAlienHuman/eliot-search/pull/610) is not merged by these docs.

| Need | Start here |
|---|---|
| Product and boundaries | [Architecture](docs/architecture/README.md), [local scope](docs/architecture/LOCAL_SCOPE.md) |
| Implement | [Work register](docs/audit/ANALYSIS_REGISTRY.md#work-register) → [delivery sequence](docs/implementation/README.md#delivery-sequence) |
| Cleanup decisions for every package | [Workspace map](docs/audit/WORKSPACE_MEMBER_DISPOSITION_2026-10-21.md#package-register) |
| Find code and check a change | [Code map](docs/implementation/CODE_MAP.md), [working guide](docs/implementation/WORKING_GUIDE.md) |
| Find a specific rule | [Documentation index](docs/README.md) |
| Run | [Quickstart status](QUICKSTART.md); no verified persistent quickstart yet |

Implement root/settings first, then one file through retained storage and Qdrant to an actual CLI excerpt.
Add restart/inventory and complete-generation convergence next. Do not build optional systems first.

## License

See [LICENSE](LICENSE).
