# Working on the CLI

The CLI owns native argument parsing, local help/version, daemon-child selection/capture, bounded output, and closed response validation for the root operations and retain-file. Keep source paths as native OsString/PathBuf and forward them to the same owned daemon child.

The retained-file result is metadata about a verified ObjectRef/current row, not a query or search result. Do not restore a temporary line protocol or legacy result handlers. Runtime qualification is owned by the designated verifier.
