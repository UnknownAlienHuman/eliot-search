# Rust-only validator entrypoint boundary

T41 required tooling is owned by Rust/Cargo commands. PowerShell files under
`tools/` are compatibility wrappers or bounded validators; none may launch
Python or Node. Required workflow files remain manual-only and read-only.

Run:

```powershell
cargo test --locked -p xtask --test tooling_runtime_boundary --test tooling_runtime_inventory
```

or the compatibility wrapper:

```powershell
pwsh -NoProfile -File tools/validate-rust-only-entrypoints.ps1
```

`tooling_runtime_boundary` verifies the explicit wrapper-to-`xtask` mappings and
rejects restoration of retired validators. `tooling_runtime_inventory` walks
all executable files under `tools/` and `.github/workflows/`, rejects
Python/Node-family source files and runtime manifests, rejects executable
Python/Node commands, and fails closed on symbolic links.

A passing run is structural T41 evidence only. It does not issue authority,
qualify Qdrant, prove native Windows behavior or replace the workspace/runtime
regression gates.
