# search-config: current functions and replacement

Read the [implementation guide](../../docs/implementation/README.md). Source was inspected at public
`c8e38b6fc9829f1cd38070773b1b3e5b21de18ad`; this is an implementation map, not a delivered runtime claim.

## Current call path

[process::effective_from_process](../../bins/eliot-searchd/src/config_composition/kernel/process.rs)
constructs defaults, captures file/environment/CLI documents, calls `build_effective`, then
[try_activate](../../bins/eliot-searchd/src/config_composition/kernel/activation.rs).
[Runtime startup](../../bins/eliot-searchd/src/public_runtime_service/kernel/runtime.rs) and
[status commands](../../bins/eliot-searchd/src/app/kernel/status.rs) call this composition.

| Current owner/function | Actual responsibility | Replacement |
|---|---|---|
| `parse_document`, `ConfigDocument::from_entries` in `document` | Restricted TOML events and generic typed entry admission | Decode the finite file settings directly with `toml::from_slice`; typed overrides share the same field validation. |
| `register_sections`, `merge_layers`, `project_section` | Generic descriptors/maps and precedence | Keep supported precedence in a small merge over real setting structs; unknown fields/types are errors. |
| `ValidatedSection::new`, `assemble_effective`, `fingerprint` | Hash sections, provenance and effective snapshots | Ordinary values/settings revision where a consumer needs equality. No hash as permission to construct an owner. |
| `diff`, `plan_reconfiguration`, `ReceiptKind` | Generic delta, ordered obligations and receipt labels | Direct supported runtime update or explicit restart; profile rebuilding uses publication. Remove unused lifecycle machinery. |
| Daemon `capture_*` in `capture.rs` | Re-encode file/env/CLI provenance and compute digests | Parse once into typed input. Retain a useful field/source label for errors, not a canonical hash transcript. |
| Daemon `build_effective` in `snapshot.rs` | Rebuild registry; project every section; require `ValidatedSecretsConfig` | Construct only selected concrete owners from accepted settings. Ordinary local startup has no compulsory secret lifecycle. |
| `try_activate(current, candidate, proven)` | Check a caller-supplied set of receipt **kinds** | Remove from startup. Enum membership does not execute or observe a runtime change. |

## Startup: accept settings, then construct

The actual daemon registry currently declares `query.limit` as an integer, default 10, allowed from
file/environment/CLI and classified `ApplyLive`. `effective_from_process` passes an empty receipt set
to `try_activate`, which demands `LiveApply` for a changed limit. The existing
`startup_partial_refuses_and_retains_defaults_without_leaks` test explicitly expects that refusal.
This is source evidence of the old behavior, not a successful launch test.

Capture invocation once → read selected inputs → decode/merge → validate real values → construct the
application. There is no running defaults-only instance to reconfigure at first launch. Pass the accepted
limit to the real query handler; accepting or printing it alone is insufficient.

For a running application, implement only an actual supported operation. Keep one applied settings
value with the owner. If an option requires restart, report that plainly. A profile change that rewrites
derived data uses the existing generation/publication owner and its actual pending work. Do not store
another generic config intent, effect DAG or readback proof beside it. Status reads existing runtime
settings and component state; it does not reread argv, environment or file and activate a new snapshot.

## Capture corrections at the application edge

| Source code | Failure / unnecessary work | Concrete correction |
|---|---|---|
| `capture_process_environment` uses `std::env::vars()` before prefix filtering | Any unrelated non-Unicode environment entry can panic during iteration | Use `vars_os`; select `ELIOT_SEARCH__` names before converting selected textual values. Invalid selected config values return an error. |
| `strip_config_args_os` converts `--config-file` to `str` | Rejects an otherwise usable non-UTF8 path; duplicates the String argument parser | One invocation parser; keep filesystem paths as `OsString`/`PathBuf`, textual option values as text. |
| `read_config_file_bytes`: metadata check → `std::fs::read` → length check | File growth/replacement after metadata can allocate/read past the intended ceiling | Open once, check the opened object as needed, read at most `limit + 1` bytes and reject overflow. `Read::take` bounds bytes, not waiting time. |

These are ordinary standard-library operations, not a new acquisition protocol or custody framework.

## TOML donor and useful checks

The locked `toml 1.1.7` provides `from_slice::<FileSettings>` / `from_str::<FileSettings>` with Serde.
Use `std,serde,parse`, as the existing `xtask` dependency already does. Add direct dependencies to the
actual production owner and remove its replaced event receiver/builder. Keep the donor's default
recursion guard; do not enable `unbounded`. See [source and tradeoff](TOML_PROFILE_2.md).

Use optional typed file/override fields where needed to distinguish absence from an explicit value;
merge defaults once, reject unknown fields and validate actual ranges. Do not re-create `ConfigValue`,
a section-provider registry or a custom TOML grammar around the donor.

During implementation, check that a non-default query setting reaches its consumer, supported layer
precedence works, unknown/range-invalid fields fail before construction, an unrelated OS environment
entry cannot panic startup, and a growing file cannot bypass the read ceiling. Exercise a real supported
runtime update separately from startup. These docs-only edits did not execute those cases.
