# Package-map and relationship coverage v2

Reconcile and validate the reviewed relation graph with the bounded Rust tooling:

```powershell
cargo run --locked --quiet -p xtask -- generate coverage-graph --check --json
cargo run --locked --quiet -p xtask -- generate package-maps --check --json
cargo run --locked --quiet -p xtask -- validate coverage-graph --json
cargo run --locked --quiet -p xtask -- validate package-maps --json
```

Coverage routes are explicit reviewed registries. The generator updates only derived manifest metadata and the human report; it does not assign ownership through lexical similarity or package-name heuristics. A new or changed operation, documentation node or dependency route requires an explicit registry change and review.

The suite checks the complete bidirectional path:

```text
document / principle / operation
→ exact reviewed package-local module
→ Cargo package
→ Cargo dependency and accepted public handoff
→ architecture / configuration / recipe / port / schema relations
```

It also verifies that every package has one bounded four-file map bundle, every map remains below ten thousand lines, workspace members and internal Cargo dependencies match the machine package registry, and the package dependency graph is acyclic and wave-monotonic.

A PASS is static design evidence only. It does not authorize implementation, issue a ticket or lease, accept a package, accept a gate/wave, or prove runtime behavior.
