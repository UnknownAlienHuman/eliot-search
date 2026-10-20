# Historical coverage graph v2

This document described a generated ownership graph for the retired Architecture 8.4 package/wave programme:
45 packages, hundreds of declared modules and operations, thousands of Markdown nodes, reviewed relation
routes and committed package-map bundles.

It is not a current architecture, implementation requirement or merge gate. The current architecture is a
short manifesto and the implementation guide assigns real owners/callers directly. Package/module/heading
routes are allowed to disappear as the corresponding systems are deleted or merged.

Retire together:

```text
xtask architecture-coverage / architecture-coverage-contracts
coverage-graph generator and validator
package-map generator and validator
swarm/coverage registries and generated package-maps
PowerShell wrappers and manual workflows
parity/module-ownership/source-shape fixtures
```

Do not regenerate the graph, convert it to another format or require explicit route review for ordinary code
changes. Use Cargo metadata, compiler visibility, direct source/API review and actual product behavior.

Historical generated files remain available in Git history. Current tooling disposition is
[`../../xtask/README.md`](../../xtask/README.md) and [issue #214](https://github.com/UnknownAlienHuman/eliot-search/issues/214).
