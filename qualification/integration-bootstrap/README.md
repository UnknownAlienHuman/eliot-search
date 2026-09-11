# P00 integration-bootstrap qualification

Run the Rust validator from the repository root:

```powershell
cargo run --locked -p xtask -- validate integration-bootstrap --json
```

The PowerShell compatibility entrypoint invokes the same Rust command:

```powershell
pwsh -NoProfile -File tools/validate-integration-bootstrap.ps1 -Json
```

For lockfile preview mode, `-AllowMissingLock` omits Cargo's `--locked` flag so Cargo can materialize a candidate lockfile before validation and printing. It does not weaken verification mode.

The validator checks the exact Rust toolchain, Cargo aliases, frozen build-profile set, dedicated data-layout invariants, workspace resolver/member count/edition, Cargo.lock format and manual read-only workflow policy.

A PASS is structural bootstrap evidence only. It does not issue control records, accept any package/gate/wave, advance launch state, prove runtime behavior or claim product acceptance.
