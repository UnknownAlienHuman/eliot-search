# Package-map and relationship coverage v2

Run after regenerating the relation graph:

```powershell
python tools/generate-coverage-graph-v2.py
python tools/generate-package-maps-v2.py
python tools/validate-coverage-graph-v2.py --json
python tools/validate-package-maps-v2.py --json
```

The suite checks the complete bidirectional path:

```text
document / principle / operation
→ exact package-local module
→ Cargo package
→ Cargo dependency and accepted public handoff
→ architecture / configuration / recipe / port / schema relations
```

It also verifies that every package has one bounded four-file map bundle, every map remains below ten
thousand lines, workspace members and internal Cargo dependencies match the machine package registry,
and the package dependency graph is acyclic and wave-monotonic.

A PASS is static design evidence only. It does not authorize implementation, issue a ticket or lease,
accept a package, accept a gate/wave, or prove runtime behavior.
