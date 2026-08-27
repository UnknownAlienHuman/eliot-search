# search-domain

Pure domain cores.

State machines, validation, ranking rules, reconciliation and policy expressed as pure functions over
`search-contracts` types.

- **Owns:** deterministic decision logic and invariant tests.
- **Must not own:** I/O, clocks, process handles, vendor clients.
