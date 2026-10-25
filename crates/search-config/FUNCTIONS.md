# Typed settings — incremental consumers, not a configuration platform

The current generic parser/registry/fingerprint/receipt stack parses many fields but applies few of them. The
replacement is a small typed merge used directly by constructed owners. This file owns capture, precedence,
validation and redacted status; each operational owner owns the meaning of its fields.

## Critical sequencing rule

#333 is an umbrella. Do **not** implement every future setting before its consumer exists.

### Foundation fields shipped with root work

```text
BootstrapSettings {
  data_root: PathBuf,
  storage_profile: Plain,
  config_input_limit,
  diagnostic_limit,
}
```

The first implementation parses invocation/config once, validates these values, constructs the root owner and
reports factual status.

### Fields added later with their owner

| Slice | Add only when consumed |
|---|---|
| Source walking/reconciliation | selected roots, ignore/admission policy, source limits |
| Qdrant/publication | endpoint, optional actual API key, generation/schema limits |
| Query/session | default result/read/output/window limits |
| Optional encrypted profile | accepted #598 profile/key settings |
| Optional donors | only the exact selected consumer settings |

A field with no constructed consumer is rejected or absent; it is not accepted into a dormant snapshot.

## Target capture

```text
capture startup cwd
→ parse args_os once into command + CLI layer
→ resolve explicit config path
→ open once and read at most max_file_bytes + 1
→ decode closed typed file layer
→ inspect only the selected ELIOT_SEARCH__ environment namespace
→ merge defaults < file < environment < CLI
→ validate effective values once
→ construct owners
```

Use `OsString`/`PathBuf` for native path inputs. TOML text is UTF-8; do not invent a byte escape format. Resolve
relative file paths against the config file parent and CLI/environment paths against captured startup cwd.

## Layer semantics

- every layer field is optional;
- omission inherits;
- `false`, `0` and `[]` are explicit values;
- a supplied list replaces the lower list; it never appends implicitly;
- repeated scalar CLI options are errors;
- malformed lower layers fail even when a higher layer would override them;
- unknown nested keys, duplicate definitions, wrong types and unrepresentable values fail;
- domain ranges and checked unit conversions run after merge;
- unrelated environment entries are ignored without decoding their values;
- unknown names inside `ELIOT_SEARCH__` fail.

Use high-level TOML + Serde only with the consuming code change. Close every nested struct with
`deny_unknown_fields`; do not combine it casually with `flatten`. Bound bytes and list counts before decode.

## Diagnostics and secrets

Diagnostics identify the source, field/error class and bounded location when available. They do not dump argv,
environment values, config excerpts, credentials or secret material. Secret bytes are owned by concrete
Qdrant/restricted-client/encrypted-storage owners, not by a generic settings snapshot.

## Status and runtime changes

Status serializes actual retained settings and observed owner state:

```text
root opened?
object/control ready?
Qdrant connected and selected generation verified?
PublishedView installed?
handler constructed?
```

No fingerprint or accepted receipt can turn false into true.

Baseline settings are startup-only. Add live change only for a real owner with a direct safe operation. Query
defaults may later be atomic for new requests. Source selection triggers reconciliation; Qdrant endpoint,
root/storage profile and schema/profile changes require restart or a new generation.

## Preserve during cutover

The current locator hints used by a live caller remain bounded relocation hints until that caller moves. They do
not admit a source or define membership. Coordinate only the exact DEK-selection consumer with #598; unrelated
plain-storage settings work proceeds now.

## Remove with callers

Delete the generic document/value/registry/section/snapshot/fingerprint/diff/action/plan/redaction APIs,
receipt-gated activation/readiness, duplicate process parsing, manual fingerprint-heavy status and fixtures whose
only subject is that platform. If the remaining typed merge is small, keep it beside daemon composition rather
than preserving a facade crate.

## Focused checks after code

- args/config captured once;
- root/plain settings reach the real root/object constructors;
- file < env < CLI precedence and explicit empty/false behavior;
- malformed/unknown/duplicate input fails before owner effects;
- native path handling is lossless for supported OS-valued inputs;
- oversized file/list/value input is bounded;
- diagnostics redact content;
- status reflects real owners;
- adding a later field and its consumer happens in the same implementation cut.
