# Working on search-config

Read the [architecture](../../docs/architecture/README.md),
[implementation guide](../../docs/implementation/README.md), package [README](README.md) and
[source audit](FUNCTIONS.md).

Do not repair the generic configuration platform. Replace it with concrete typed settings used by actual
owners.

Select and pin high-level `toml` + Serde during the typed-settings cutover; the current lock has only
`toml_parser`, not those target dependencies. Use a bounded file, `vars_os()` for selected environment fields and one
`args_os()` parser. Keep paths as OS strings. Merge supported optional fields over defaults and validate
final values once. Unknown fields/types/ranges fail before constructing any owner.

Startup constructs the application directly. Remove `try_activate`, fingerprints, registry/section digests,
action DAGs, receipt kinds, mandatory secret section and accepted-readiness receipts. Do not manufacture a
`LiveApply` receipt to accept startup values.

Retain only supported concrete settings:

- root/source locations and real source policy fields;
- storage profile;
- Qdrant endpoint/optional managed settings;
- lexical/retrieval profile;
- query defaults;
- selected optional profiles with real handlers.

Implement hot reload only for a concrete owner operation. Otherwise require restart. Profile/schema changes
use publication; external uncertainty remains with that owner. Persist one current typed value only when
restart must remember a runtime-applied change—never a completed config operation history.

Status serializes running settings and actual component readiness. It does not rebuild/redact a generic
snapshot, hash it or require acceptance booleans.

Change daemon capture/construction/status and setting consumers together, then delete generic parser/tree/
registry/snapshot/fingerprint/diff/plan/redaction APIs and their fixtures. If little shared code remains,
delete this crate rather than preserve a facade.

After code run minimal Rust 1.98 compiler/Clippy for actual config/control/daemon/consumer targets and focused
precedence/bounds/non-Unicode/file-ceiling/status/live-vs-restart cases.
