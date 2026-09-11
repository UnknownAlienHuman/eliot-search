# W1 milestone packet qualification

Run the Rust validator directly:

```powershell
cargo run --locked -p xtask -- validate w1-milestone-packets --json
```

The PowerShell compatibility entrypoint invokes the same Cargo command:

```powershell
pwsh -NoProfile -File tools/validate-w1-milestone-packets.ps1 -Json
```

A PASS proves only bounded packet topology: seven packages, four ordered checkpoints per package, package-only scopes, exact dependency handoffs, blocked G0/W0 state and manual workflow policy. It does not authorize a writer or accept any implementation.
