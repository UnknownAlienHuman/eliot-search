# Working on xtask and repository tooling

Read [README.md](README.md), [issue #214](https://github.com/UnknownAlienHuman/eliot-search/issues/214)
and [issue #525](https://github.com/UnknownAlienHuman/eliot-search/issues/525).

Do not repair, regenerate or migrate architecture coverage, package maps, accepted-evidence digests,
canonical-digest ledgers, build/wave profiles, source-shape/module-ownership tests or the partial Rust/Qdrant
analyser. Remove each policy, command, wrapper, workflow, registry, generated output and exclusive fixture
with its last consumer.

Keep only direct tools which survive real implementation:

- Cargo metadata/locked compiler and minimal Clippy for changed owners/consumers;
- focused behavior/live checks;
- optional direct lockfile regeneration diff;
- lightweight documentation/diff checks.

Do not create `xtask-v2`, a new approval database, AST/rustdoc/taint framework, service locator, controller,
acceptance receipt or archive copy. If no substantial custom operation remains, delete the xtask package and
use direct Cargo/Git commands.

The compiler and actual public API enforce Rust boundaries. Qdrant behavior is verified through the connected
bridge/publication/query path. A generated map, digest or source spelling is not product evidence.

After code/tooling deletion, run minimal Rust 1.98 compiler/Clippy for the remaining tooling and immediate
workflow consumers. Broad historical validator suites are removed with the policies they protect.
