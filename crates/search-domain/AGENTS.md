# Working on search-domain

Read the affected row in [FUNCTIONS.md](FUNCTIONS.md) and the receiving owner's recipe. The old caller census is
historical; inspect real imports, features and dispatch before moving or deleting a family.

Keep effect-specific decisions with their owner. Do not wrap them in another Decision/reason/fingerprint layer
or combine different recovery semantics to obtain a shared helper. Follow the root
[implementation choices](../../docs/implementation/WORKING_GUIDE.md#implementation-choices).

Change the selected path and its immediate consumers; remove displaced exports/fixtures/dependencies with it.
Run minimal locked check/Clippy for that closure. Delete the package after the final useful caller moves, not
before; later behavioral checks belong to the receiving operation.
