# Working on the CLI

The CLI owns native argument parsing, local help/version, daemon-child selection/capture, bounded output, and closed response validation for root, retain-file and prepare-source operations. Keep paths native and preserve the first parse's source ID/unit binding; never recover selectors from overlapping raw-argument windows.

Retain-file reports a verified ObjectRef/current row. Prepare-source reports actual stored unit text/ranges with the serialization-derived bound in [FUNCTIONS.md](FUNCTIONS.md). Neither is a search result. Preserve the existing owned child and cleanup; do not restore a temporary protocol or legacy result handlers. Runtime qualification belongs to the designated verifier.
