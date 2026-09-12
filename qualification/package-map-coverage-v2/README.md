# Package-map and relationship coverage v2

Regenerate and validate the relation graph with the bounded tooling entrypoints:

```powershell
python tools/generate-coverage-graph-v2.py
cargo run --locked --quiet -p xtask -- generate package-maps
cargo run --locked --quiet -p xtask -- validate coverage-graph --json
cargo run --locked --quiet -p xtask -- validate package-maps --json
```

The Python command is temporarily retained only for graph derivation/generation. Coverage-graph and package-map validation are Rust-owned and require no Python runtime.

The suite checks the complete bidirectional path:

```text
document / principle / operation
→ exact package-local module
→ Cargo package
→ Cargo dependency and accepted public handoff
→ architecture / configuration / recipe / port / schema relations
```

It also verifies that every package has one bounded four-file map bundle, every map remains below ten thousand lines, workspace members and internal Cargo dependencies match the machine package registry, and the package dependency graph is acyclic and wave-monotonic.

A PASS is static design evidence only. It does not authorize implementation, issue a ticket or lease, accept a package, accept a gate/wave, or prove runtime behavior.
