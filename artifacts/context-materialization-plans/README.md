# Context materialization plans

Ignored ordinary local output from the Rust planner:

```powershell
cargo run --locked --quiet -p xtask -- build context-materialization-plan ...
```

A directory contains `plan.json`, an optional prospective signed-payload TOML file and, only when both
external signatures are present and valid, an optional complete prospective manifest TOML file. These
files are not control records, immutable artifact refs, signature artifacts, tickets, leases, accepted
evidence or implementation authority.
