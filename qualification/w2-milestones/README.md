# W2 milestone packet qualification

Run the Rust validator directly:

```powershell
cargo run --locked -p xtask -- validate w2-milestone-packets --json
```

The PowerShell compatibility entrypoint invokes the same Cargo command:

```powershell
pwsh -NoProfile -File tools/validate-w2-milestone-packets.ps1 -Json
```

A PASS proves only bounded checkpoint topology: eight packages, four ordered checkpoints per package, package-only scopes, exact dependency handoffs, daemon re-entry replacement, DIRECT-only W2 context, blocked G0/W1 state and manual workflow policy. It authorizes no writer and accepts no implementation.
