# Qualified profile identifier registry implementation

**Verdict: ACCEPTABLE for integration code/registry publication**, at independently reviewed source
`7d15b382add5e40630035bf2e4d5243738ae2b61`. Integrated commit
`3e763ee9cd9f245cc1275149029624e82672f227` preserves all nine reviewed Git blobs exactly.

The canonical registry now declares one `QualifiedOpaqueId` string type and has 52 types. Its exact
ASCII grammar, 96-byte namespace, 128-byte local component, one slash and 225-byte total bound match
the separately accepted syntax decision. The existing `OpaqueId`, two profile-reference rules,
canonical field orders, registry/schema versions and record layouts are preserved. The new bounded
checker validates the definition and those two owner-specific bindings and uses the pure parser for
field-value syntax. Namespace ownership, profile existence, trust and qualification are not inferred.

One combined Cargo invocation at clean source passed all 25 selected tests: enum bindings 7/7,
instance profiles 8/8, qualified-ID registry 7/7 and ticket builder 3/3. One PowerShell structural
invocation exited zero with 52 types, 8 records, 211 fields, 9 signature refs, 28 manual workflows,
zero issued records and no errors. Rust toolchain `+1.98.0` was requested with `--locked --offline`.
The capture records source/parent, clean states, commands, worktree, timestamps, environment, exits
and original stdout/stderr. Independent review checked their byte counts and SHA-256 and the raw
outcomes. No command was rerun after integration.

The [original metadata](evidence/qualified-profile-id-registry/metadata.json) has two empty objects
before each named command entry; these are preserved and are not counted as commands. The
[copy manifest](evidence/qualified-profile-id-registry/copy-manifest.json) records byte-exact copying
of the five original capture files. Evidence is stored with `-text` attributes. Cargo emitted
11 warnings in other modules, whose earlier baseline was not established, and one unused import
warning in the new test. No warning-free build is claimed.

These checks prove the bounded identifier/registry implementation. They do not prove complete
control-record instance validation, provision a profile, establish authority, issue an assignment,
qualify Qdrant, or demonstrate a running Search process.
