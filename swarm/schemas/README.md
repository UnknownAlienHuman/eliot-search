# Swarm control-record schemas

These TOML files define field-level machine contracts for integration-owned assignment control records.
They are schemas, not record instances and not implementation authorization.

## Registries

`types-v1.toml` schema version 2 is the closed type registry. Every `kind` used by a record schema must
resolve to either:

- one of the explicit built-in scalar kinds: `bool`, `u16`, `u32`, `u64`; or
- exactly one named `[[type]]` entry in `types-v1.toml`.

Unknown kinds, duplicate names, alias cycles and implicit string/null/map coercion fail closed.

The registry separates four closed semantic surfaces:

- `ClosedReasonCode` — operation failures only;
- `LeaseEventReasonCode` — acknowledgement/submission/revocation/supersession lifecycle events;
- `SupersessionReasonCode` — append-only record replacement reasons;
- `ConsumerActionCode` — compatibility follow-up actions.

Successful lifecycle events and compatibility instructions are not represented as operation failures.

For a field path ending in `[]`, `kind` names the element type, not another list. A whole-collection type
such as `OrderedDigestSet` is valid only on a path without the `[]` suffix. This prevents accidental
list-of-list contracts.

## Record sequence

```text
context_manifest_v1
→ assignment_ticket_v1
→ writer_lease_v1
→ lease_event_v1: ACKNOWLEDGED / WRITER_ACKNOWLEDGED
→ package_submission_v1
→ independent_review_v1
→ package_handoff_v1
```

Revocation, cancellation and replacement use append-only `lease_event_v1` and
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

Unknown load-bearing fields are rejected. Concrete record files use the top-level group order and field
order declared by their schema and UTF-8/LF exact-byte canonicalization. TOML `null`, field omission as
optional semantics and open reason/action values are rejected.

## Digest rule

V1 record schemas retain the field name `signature.record_sha256`. It is not a self-hash of the complete
file. It means the signed payload SHA-256 over exact bytes before the `[signature]` table.

A consuming record or operation receipt separately stores:

```text
repository and immutable commit
repository-relative path
Git blob identity
exact_record_file_sha256 over the complete committed file
closed record kind
```

These are different types and values. Full details are normative in
`swarm/RECEIPT_CANONICALIZATION.md`. A validator rejects any implementation that attempts an embedded
fixed-point complete-file digest.

## Record paths

```text
swarm/context-manifests/<package>/<context_record_sha256>.toml
swarm/tickets/<package>/<ticket_id>.toml
swarm/leases/<package>/<lease_id>.toml
swarm/leases/<package>/events/<event_id>.toml
swarm/submissions/<package>/<submission_id>.toml
swarm/reviews/<package>/<review_id>.toml
swarm/handoffs/<package>/<handoff_id>.toml
swarm/supersessions/<record_kind>/<receipt_id>.toml
```

The package handoff path uses unique `handoff_id`. The API schema digest is accepted public-surface
identity, not record/path identity. Multiple append-only handoffs may therefore retain the same API
digest while binding different reviewed commits or evidence.

## Validation

Run manually from a Windows PowerShell environment:

```powershell
pwsh -NoProfile -File tools/validate-ticket-issuance-contracts.ps1
pwsh -NoProfile -File tools/validate-ticket-issuance-contracts.ps1 -Json
```

The validator checks type closure and aliases, exact failure/lifecycle/action registries, schema/record
parity, field-group and field order, element-vs-collection semantics, digest rules, exact path parity,
orchestration/launch bindings, zero state and every repository workflow's manual/read-only policy.

A PASS is structural only; it does not issue a ticket, create a lease or accept a package.

## Content floor

Control records may contain repository-relative paths and immutable refs/digests. They contain no source
body, unsaved buffer, raw query, credential/token, local absolute path, provider internals or dependency
implementation source. Raw evidence remains in separately governed immutable artifacts.

## Non-claims

- no context materializer or ticket issuer implementation exists;
- no draft is converted to a record;
- no package has an issued ticket or active lease;
- no schema PASS is package, G0 or W0 evidence;
- package writers cannot edit schemas or integration records.
