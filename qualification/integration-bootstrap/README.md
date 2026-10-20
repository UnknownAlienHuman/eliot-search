# Historical P00 integration-bootstrap qualification

This directory qualified the retired P00/W0 repository structure. Its validator freezes:

- exactly 45 workspace members;
- five Architecture 8.4 build/wave profiles;
- exact Cargo aliases;
- seven data-root directories;
- `CONTROL_JOURNAL_ONLY`, migration quarantine and paired backup assumptions;
- one dedicated manual workflow.

Those assumptions contradict the current refactor, which deletes/merges packages, replaces the generic
control journal, removes migration/quarantine platforms and uses a simpler current root.

Delete the validator, workflow, wrapper, frozen build/layout registries and exclusive fixtures under
[#214](https://github.com/UnknownAlienHuman/eliot-search/issues/214). Rust/Cargo already provide the useful
facts directly:

```text
rust-toolchain.toml
Cargo.toml
Cargo.lock + --locked
cargo metadata
focused cargo check / clippy
```

Do not replace the bootstrap with a new fixed package count, profile registry, acceptance report or launch
state. Historical records remain in Git.
