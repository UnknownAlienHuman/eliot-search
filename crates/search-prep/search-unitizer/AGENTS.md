# Agent contract — search-unitizer

You own only `crates/search-prep/search-unitizer/`. Do not edit another package, the root workspace, or shared contracts.
When a missing contract blocks correct work, open a contract-change issue with the exact field,
invariant, producer, consumer and compatibility impact; do not patch around it.

The Architecture 8.4 master does not need to be loaded for ordinary work. This file contains the
package slice. Traceability only: S7.5-S7.6, S11.3, S17, S24, P04, P06.

## Mission

Turn a materialization into deterministic unit occurrences and an immutable unit manifest.

## Ownership

- unitizer profiles
- UnitOccurrence creation
- native anchor preservation
- ordinal/structural identity rules
- unit manifest digest and determinism

## Forbidden ownership

- ranking
- assuming unit stability across arbitrary reparses
- compiler certainty
- Qdrant point transport
- opening source stores directly instead of consuming immutable contract inputs

## Allowed dependencies

`search-contracts`, `search-domain`. Additional internal or external dependencies require an explicit boundary review. Public
APIs may expose only `search-contracts` or package-owned opaque types; vendor types stay private.

### #257 materialization ingress exception

The [accepted #257 integration decision](https://github.com/UnknownAlienHuman/eliot-search/issues/257#issuecomment-6093710492)
allows an internal `search-materializer` dependency and the single `prepare_unit_set_input` adapter to
consume its immutable product, validated request and profile. This adapter calls the existing owning
materialization verifier; it must not duplicate coordinate/loss-map validation. Peer types may appear
only at this named ingress. UnitSet results use contract types and package-owned opaque types.
No materializer, daemon, planner or storage implementation changes are authorized by this exception.
The unchanged legacy DIRECT profile/layout APIs remain until #331 and never create or verify a v3 set.

### #440 Markdown ingress exception

The [frozen #440 slice](https://github.com/UnknownAlienHuman/eliot-search/issues/440) adds the named
`MarkdownUnitizationSource` adapter over the existing dependencies. Its borrowed Markdown product,
request, profiles and revision port are untrusted inputs to `unitize_markdown` and
`verify_markdown_unit_manifest`. The adapter calls public owning `verify_markdown`, then the unchanged
raw v3 prepare/build path. It preserves the original AtomicBool and absolute Instant across both.
Peer types appear only in that named ingress record; structural results expose package-owned views
and contract Representation/UnitOccurrence. No parser/map verifier, admission authority, storage,
worker, daemon or consumer behavior is added. Raw v3 stays byte-compatible; the separate structural
v4 profile requires VERSIONED_REBUILD on #331 adoption. JATS has its separately frozen #446 ingress.

### #446 JATS ingress and size exception

The [frozen #446 slice](https://github.com/UnknownAlienHuman/eliot-search/issues/446) adds only the
named borrowed `JatsUnitizationSource` over existing dependencies. `unitize_jats` and
`verify_jats_unit_manifest` call owning `verify_jats`, then unchanged raw-v3 prepare/build, with the
original AtomicBool and absolute Instant. Retain every element/attribute/text/comment, optional
declaration and event count; whole expected reconstruction decides completeness. Peer types appear
only in this named ingress, results use contract/package-owned immutable views. No parser/map
verifier, admission, storage, worker, daemon or consumer behavior changes. Raw/v3/Markdown/legacy
source and retained fixtures are preserved. Additive structural v5 requires VERSIONED_REBUILD on
#331 adoption; decode remains a proposal.

Independent concrete pre-Ready size review accepts simultaneous slice-only ceilings of **2,750 JATS
module lines / 9,998 whole-owner hand-written Rust lines**, retaining the hard **10,000** threshold.
The reviewed prototype is 2,711 lines including 635 private fixture lines; every final glue/lint/format
change is recounted. No inherited allowance, fixture deletion or artificial source relocation.
Finite logical work/storage limits are not CPU/RSS measurements or hard donor preemption.
Production check/strict Clippy/static gates are source evidence; #215/#216 execute the authored cases.

## Required logical surface

These are behavior contracts, not mandated Rust syntax. Preserve the semantics even if the concrete API
is improved:

- `unitize(materialization, profile) -> Result<UnitManifest, UnitizeError>`
- `derive_unit_occurrence(representation, unit) -> UnitOccurrence`
- `validate_anchor(unit, maps) -> Result<(), UnitizeError>`
- `manifest_digest(units, profile) -> Digest`

## Failure surface

Use typed errors/reason codes. Relevant public reasons: `UNITIZATION_NONDETERMINISTIC`, `ANCHOR_MAP_INVALID`, `MATERIALIZATION_LOSS`. Never turn a degraded or partial
state into an apparent success.

## Test seams and exit evidence

- `same input/profile yields byte-identical manifest`
- `unit ordinals and anchors are deterministic`
- `lossy maps cap assurance`
- `empty and malformed representations remain bounded`
- `unit IDs are occurrence-specific, not assumed globally stable`

Property/fault tests belong beside the owning behavior. Shared control-corpus fixtures may be requested,
but the writer does not edit another package opportunistically.

## Size and split guard

- Delivery wave: **W2 / P04-P06**
- Soft `src/` target: **6,500 lines**
- Hard review threshold: **10,000 total hand-written Rust lines**
- Split on a real security, runtime, replacement, test or dependency boundary; never create a forwarding
  wrapper or crate-per-type shell.

## Definition of done

The package has a vendor-neutral public contract, deterministic tests for its invariants, explicit
degradation behavior, no forbidden dependency, and a handoff reporting commands and raw outcomes.
Compilation alone is insufficient.
