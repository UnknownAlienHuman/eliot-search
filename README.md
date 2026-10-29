# ELIOT Search

A standalone local search application for codebases and document collections. The target returns ranked
locations, verified retained-source excerpts and explicit gaps. Qdrant is the derived index, not source evidence.

**Current documentation is published on `main`. The persistent product is not connected end to end.**
The source refactor in [PR #610](https://github.com/UnknownAlienHuman/eliot-search/pull/610) is separate and has not
been merged by this documentation update. The [registry](docs/audit/ANALYSIS_REGISTRY.md#resume) distinguishes
published rules, inspected source revisions and executed behavior.

## Start here

| Need | Read |
|---|---|
| Product and invariants | [Architecture](docs/architecture/README.md) |
| Implement the next connected path | [Delivery sequence and owners](docs/implementation/README.md#delivery-sequence) |
| Resume work or inspect evidence | [Analysis registry](docs/audit/ANALYSIS_REGISTRY.md#resume) |
| Find a specific rule | [Documentation index](docs/README.md) |
| Run the application | [Quickstart status](QUICKSTART.md); no verified persistent quickstart yet |

Start with root/settings, then one retained file through Qdrant to a CLI excerpt. Add restart and complete-generation
convergence next. Optional models, encryption and adapters do not block that path. Existing source is evidence of
what is implemented, not a requirement to preserve old architecture.

## License

See [LICENSE](LICENSE).
