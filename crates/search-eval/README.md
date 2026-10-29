# Legacy evaluation package

This package remains in the main source baseline. Its removal in PR #610 is not delivered by the documentation
landing. This README remains available to `src/lib.rs`'s `include_str!`.

Do not extend the old evaluation/receipt platform or treat it as runtime authority. Follow the current
[implementation](../../docs/implementation/README.md) and [test cutover](../../docs/implementation/TEST_CUTOVER.md).
Remove this package only with its actual consumers and minimal compiler/Clippy for the connected source cut.
