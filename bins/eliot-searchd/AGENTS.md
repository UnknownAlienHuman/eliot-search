# Working on the daemon

Read [FUNCTIONS.md](FUNCTIONS.md), the current delivery stage and only affected owner recipes.
The daemon composes one application: invocation/root, source/object/control, preparation, Qdrant/publication,
query/evidence and one dispatcher. It does not duplicate those owners' algorithms.

Trace current interceptors/callers and remove the displaced route with its consumer. Enabling old wave features
or adding another dispatcher does not connect the product. Do not build receipts, grants or qualification
constructors around missing effects. Preserve user data and unpublished #598 work.

The root recipe alone owns platform finalization; the lexical/bridge recipes own Qdrant profiles and SDK mapping.
Use execution-only context outside query handling. Follow the implementation lock order.

After production changes, run minimal locked Rust 1.98 check/Clippy for affected consumers including the actual
eliot-searchd binary. Library checks, help and self-test strings do not prove daemon behavior.
