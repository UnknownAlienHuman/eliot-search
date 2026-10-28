# Working on index cleanup

This package is a removal target. Read [FUNCTIONS.md](FUNCTIONS.md) and the
[publication cleanup rule](../search-publication/FUNCTIONS.md#view-lifetime-and-whole-generation-cleanup).
Move only the exact whole-collection deletion/absence helper with its real callers.

No individual-point physical deletion, point manifests, cleanup cursors, watermarks, receipts or separate
reclaimer journal in Stage 2/3A or conditional Stage 3B. Closed Stage 3B points remain until whole-generation
retirement. Local-object GC is separate stopped-root maintenance.

Run minimal check/Clippy for actual publication/bridge/control consumers after code; do not qualify the retired
framework or preserve its exclusive fixtures.
