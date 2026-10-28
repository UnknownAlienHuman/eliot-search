# ELIOT Search

A standalone local search application for codebases and document collections. The target product returns ranked
locations, verified excerpts, exact retained revision identity and explicit gaps. Qdrant is the derived search
index; immutable objects remain the evidence source.

**Status: the persistent product is not connected end to end.** New-generation documentation and refactoring
are in [PR #610](https://github.com/UnknownAlienHuman/eliot-search/pull/610), branch
`codex/refactor-preparation-20261017`. Until that PR is merged, `main` still carries the previous generation.
Check the branch before following instructions; this page does not claim the target behavior already runs.

## Start here

| Need | Read |
|---|---|
| Product and invariants | [Architecture](docs/architecture/README.md) |
| Implement the next connected path | [Delivery sequence and owners](docs/implementation/README.md#delivery-sequence) |
| Resume work or inspect evidence | [Analysis registry](docs/audit/ANALYSIS_REGISTRY.md#resume) |
| Find a specific rule | [Documentation index](docs/README.md) |
| Run the application | [Quickstart status](QUICKSTART.md); no verified persistent quickstart yet |

The implementation sequence starts with root/settings, then one retained file through Qdrant to a CLI excerpt.
Restart and complete-generation convergence follow; point-level optimization requires measured need.
Optional models, encryption and adapters do not block that path.

Build results, platform evidence and unfinished work live only in the registry. Algorithms belong to their
owners, not this introduction.

## License

See [LICENSE](LICENSE).
