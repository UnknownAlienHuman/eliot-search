# Working on the CLI

The CLI owns native parsing, local help/version, first-parse bindings and owned-child lifetime. Root, retain-file,
prepare-source and publish-prepared use argv operations; search uses the existing framed QueryClient.
Native path values may equal option names; never reconstruct selectors by rescanning overlapping raw arguments.

Publication returns generation state/counts only and has no --query/--limit route. Search returns complete
verified retained units and citations from the daemon's pinned view. Preserve full units, whole-card OutputLimit,
8 MiB total framing, safe JSON presentation and actual child exit/reap in [FUNCTIONS.md](FUNCTIONS.md).
Do not restore raw-score projectors, arbitrary handles or caller-created evidence authority.
