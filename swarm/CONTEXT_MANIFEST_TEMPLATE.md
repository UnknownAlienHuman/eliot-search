# Materialized writer context manifest

The integration owner creates this manifest from one non-claimable context draft at one exact base
commit. The writer receives the resulting immutable artifact, not arbitrary repository access.

## Identity

- Context manifest ID:
- Package / stage / wave:
- Base commit:
- Source context draft path / digest:
- Materialization mode:
- Materialized artifact ref:
- Materialized artifact SHA-256:
- Writer-visible artifact count:
- Created at:

## Source files

| Order | Repository path | Blob identity | SHA-256 | Bytes |
|---:|---|---|---|---:|

## Registry fragments

| Order | Registry path | Selector | Source SHA-256 | Fragment SHA-256 |
|---:|---|---|---|---|

## Accepted dynamic handoffs

| Dependency/prior stage | Accepted commit | Public API/config/evidence digest | Receipt ref |
|---|---|---|---|

## Canonical materialization

- UTF-8 only; reject undecodable source.
- Normalize line endings to LF only when the draft explicitly authorizes materialization normalization.
- Prefix every source file and registry fragment with its exact path/selector header.
- Preserve declared order; do not sort implicitly after draft validation.
- Include no architecture-master text, source bodies, secrets or dependency implementation source unless
  the exact draft explicitly and lawfully names them.
- Any changed source byte, selector, dynamic handoff or order creates a new manifest/artifact digest.

## Verification

- All source files existed at the exact base commit.
- Every registry selector matched exactly one record.
- Static/dynamic ceilings were respected.
- Forbidden paths/patterns were absent.
- The materialized artifact digest was independently recomputed.

## Signature

- Materializer:
- Reviewer:
- Manifest canonical digest:
- Verification receipt:
