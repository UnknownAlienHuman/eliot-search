# Swarm control-record schemas

These TOML files define field-level machine contracts for integration-owned assignment control records.
They are schemas, not record instances and not implementation authorization.

## Registries

`types-v1.toml` is the closed type registry. Every `kind` used by a record schema must resolve to either:

- one of the explicit built-in scalar kinds: `bool`, `u16`, `u32`, `u64`; or
- exactly one named `[[type]]` entry in `types-v1.toml`.

Unknown kinds, duplicate type names and implicit string/null coercion fail closed.

For a field path ending in `[]`, `kind` names the **element type**, not another list. A whole-collection
type such as `OrderedDigestSet` is valid only on a path without the `[]` suffix. This prevents accidental
list-of-list contracts.

## Record sequence

```text
context_manifest_v1
→ assignment_ticket_v1
→ writer_lease_v1
→ lease_event_v1: ACKNOWLEDGED
→ package_submission_v1
→ independent_review_v1
→ package_handoff_v1
```

Revocation/cancellation/replacement uses append-only `lease_event_v1` and
`supersession_receipt_v1`; accepted records are never edited.

## Field schema format

Each record schema contains global invariants and ordered `[[field]]` entries:

```text
path             dotted logical field path
kind             closed scalar/record/element/collection type
required         whether absence is invalid
canonical_order  contiguous field order in the exact machine record
rules            bounded semantic requirements
```

Unknown load-bearing fields are rejected. Concrete record files use the order declared by their schema
and UTF-8/LF exact-byte canonicalization.

## Digest rule

V1 schemas retain the field name `signature.record_sha256`. It is **not** a self-hash of the complete
file. It means the signed payload SHA-256 over exact bytes before the `[signature]` table.

A consuming record or operation receipt separately stores:

```text
Git blob identity
exact_record_file_sha256 over the complete committed file
```

These are different types and values. Full details are normative in
`swarm/RECEIPT_CANONICALIZATION.md`. A validator must reject any implementation that attempts an
embedded fixed-point complete-file digest.

## Record paths

The package handoff path uses the unique `handoff_id`:

```text
swarm/handoffs/<package>/<handoff_id>.toml
```

The API schema digest is accepted public-surface identity, not record/path identity. Multiple append-only
handoffs may therefore retain the same API digest while binding different reviewed commits or evidence.

## Validation

Run manually:

```powershell
pwsh -NoProfile -File tools/validate-ticket-issuance-contracts.ps1
pwsh -NoProfile -File tools/validate-ticket-issuance-contracts.ps1 -Json
```

The validator checks registry closure, schema/record parity, contiguous field orders, element-vs-
collection semantics, digest rules, append-only path layouts and zero claimable records. A PASS is
structural only; it does not issue a ticket or accept a package.

## Content floor

Control records may contain repository-relative paths and immutable refs/digests. They contain no source
body, unsaved buffer, raw query, credential/token, local absolute path, provider internals or dependency
implementation source. Raw evidence remains in separately governed immutable artifacts.

## Non-claims

- no context materializer or ticket issuer exists;
- no draft is converted to a record;
- no package has an issued ticket or active lease;
- no schema PASS is package, G0 or W0 evidence;
- package writers cannot edit schemas or integration records.
