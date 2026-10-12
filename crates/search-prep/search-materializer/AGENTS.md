# Agent contract — search-materializer

You own only `crates/search-prep/search-materializer/`. Do not edit another package, the root workspace, or shared contracts.
When a missing contract blocks correct work, open a contract-change issue with the exact field,
invariant, producer, consumer and compatibility impact; do not patch around it.

The Architecture 8.4 master does not need to be loaded for ordinary work. This file contains the
package slice. Traceability only: S7.5, S17, S24, H6, P04, P17.

## Mission

Convert an exact retained revision into a canonical representation with explicit coordinate and loss maps.

## Ownership

- materializer profile contracts
- raw text/source-code baseline materialization
- coordinate map and loss map production
- assurance ceiling classification
- provider qualification seam for optional documents
- bounded immutable legacy preparation-object/reference I/O during T02 migration
- canonical legacy preparation binding, reference and manifest wire schema
- frozen preparation digest preimages, protection tags, bounds and locator names
- legacy preparation tree/name grammar and physical/current inventory classification tags

## Forbidden ownership

- selecting a PDF/Office/OCR provider without ADR
- authority or ranking
- executing macros, archive members or remote resources
- claiming exact coordinates after lossy transforms
- opening source stores directly instead of consuming immutable contract inputs
- retained revision CAS or revision-object publication owned by `search-revision-store`
- secret acquisition, DPAPI/keyring operations or plaintext source verification
- source-registry or control-journal mutation
- daemon data-root ownership, native identity observation, directory traversal or metadata acquisition

## Allowed dependencies

`search-contracts`, `search-domain`. Additional internal or external dependencies require an explicit boundary review. Public
APIs may expose only `search-contracts` or package-owned opaque types; vendor types stay private.

## Required logical surface

These are behavior contracts, not mandated Rust syntax. Preserve the semantics even if the concrete API
is improved:

- `materialize(revision, profile, budget) -> Result<MaterializationOutput, MaterializeError>`
- `validate_coordinate_map(map, source, output) -> Result<(), MaterializeError>`
- `derive_assurance_ceiling(loss_map) -> AssuranceCeiling`
- `qualify_provider(descriptor, fixtures) -> ProviderQualification`
- `read_legacy_preparation_artifact(platform, path, maximum_bytes)`
- `publish_legacy_preparation_artifact(platform, directory, final_name, temporary_name, bytes, maximum_bytes)`
- `encode_legacy_preparation_binding(binding)`
- `decode_legacy_preparation_reference(encoded, lookup_key)`
- `encode_legacy_preparation_manifest(binding, representation, revisions, body)`
- `verify_legacy_preparation_manifest(manifest, binding, revisions)`
- `derive_legacy_preparation_lookup_key(binding, backend)`
- `derive_legacy_preparation_object_id(binding, backend, manifest_digest)`
- `classify_legacy_preparation_inventory_name(tree, name)`
- `legacy_preparation_reference_relative_locator(key)`
- `legacy_preparation_object_relative_locator(id, protection)`

The compatibility artifact adapter treats bytes as opaque. Native identity,
locator and directory-durability observations are injected. It cannot infer a
materialization profile, decrypt bytes, authorize source access or publish a
revision. A racing final artifact is reusable only after exact readback; a
conflicting artifact is never overwritten.

The legacy store schema and inventory grammar are pure. Concrete SHA-256/BLAKE3
implementations, data-root traversal, metadata and file bytes are injected;
DPAPI and filesystem operations remain composition responsibilities. The
package owns field order, fixed sizes, algorithm tags, profile revisions,
digest domains, admitted tree/final/temporary names, classification tags and
canonical relative locators. Any format/grammar change requires explicit
compatibility review rather than daemon-local reinterpretation.

## Failure surface

Use typed errors/reason codes. Relevant public reasons: `MATERIALIZATION_LOSS`, `MATERIALIZER_UNAVAILABLE`, `COORDINATE_MAP_INVALID`. Never turn a degraded or partial
state into an apparent success. Possible publication followed by failed directory durability is outcome-unknown.

Legacy preparation schema failures preserve the stable `DIRECT_PREPARATION_*`
namespace. Malformed binding/reference/manifest bytes, profile or algorithm
drift, digest mismatch and representation mismatch fail closed. Unknown tree,
upper-case/nonhex ID, unrecognized extension or malformed temporary name is not
silently reclassified as an orphan.

## Test seams and exit evidence

- `raw text round trip preserves exact byte anchors`
- `CRLF/transcoding produces explicit coordinate map`
- `lossy output cannot claim exact_bytes`
- `malformed and oversized input is bounded`
- `provider absence does not block baseline text/code`
- `legacy preparation object/reference exact replay reuses without replacement`
- `legacy preparation conflict preserves existing bytes and removes only its own temporary`
- `legacy read rejects changed identity and oversize before allocation`
- `daemon preparation paths compose this package while revision paths do not`
- `ELSPRP02 binding and ELSPRF01 reference layouts remain byte exact`
- `lookup/object digest preimages and lower-case locator names remain frozen`
- `manifest algorithm/profile tags and representation identity are verified together`
- `refs/objects tree and final/temporary filename grammar remain closed`
- `inventory classification tags and canonical relative locators remain frozen`
- `daemon contains no duplicate preparation schema constants, offsets, digest domains or filename parser`

Property/fault tests belong beside the owning behavior. Shared control-corpus fixtures may be requested,
but the writer does not edit another package opportunistically.

## Size and split guard

- Delivery wave: **W2 baseline / P04; optional P17**
- Soft `src/` target: **7,000 lines**
- Hard review threshold: **10,000 total hand-written Rust lines**
- Split on a real security, runtime, replacement, test or dependency boundary; never create a forwarding
  wrapper or crate-per-type shell.

## Definition of done

The package has a vendor-neutral public contract, deterministic tests for its invariants, explicit
degradation behavior, no forbidden dependency, and a handoff reporting commands and raw outcomes.
Compilation alone is insufficient.

## #444 inert JATS boundary review

The exact private quick-xml 0.42.0 dependency with default features disabled is accepted for the
Publishing 1.4 UTF-8 subset only. Public APIs expose package-owned immutable observations through api;
no vendor types, source admission, UnitSet, storage, schema runtime or remote execution are added.
The concrete same-owner split review independently accepts 15,150 whole-owner Rust lines and 2,400
new JATS-module lines for this slice, including private fixtures. The 10,000-line review threshold
remains in force; this exception does not inherit or widen #437 and moves/deletes no retained source.
Production check/strict Clippy and affected guards are source gates; fixtures remain
AUTHORED_NOT_COMPILED/NOT_RUN under #215/#216. JATS UnitSet and #331 adoption remain separate.

## #468 contextual legacy preparation-artifact boundary

This slice adds original-context exact read/no-clobber publication inside the existing artifact owner.
Mandatory platform callbacks borrow the same mutable caller context and meter their internal attempts;
the kernel charges filesystem work, logical capacity, byte comparisons, short/Interrupted I/O and EOF.
Contextual calls install no cleanup guard or staging unlink, including success. Redacted
PREPARATION_ARTIFACT_* failures preserve component-local attempted effects and paired platform/refusal
causes. Receipts grant no source, cleanup, recovery or enclosing-request completion authority.

The fresh #468 exception is limited to these frozen paths: whole owner <=16,500 physical Rust lines/
85 files; context subtree <=1,550 (production <=1,000, private fixtures <=550); artifact source/private
fixture family <=2,100; production churn <=1,350 and complete Rust churn <=2,000. The 7,000 soft source
target and 10,000 whole-owner review threshold remain. This exception inherits no earlier allowance.
Measured: whole owner 16,328/85; context 986+535=1,521; artifact family 1,995, including all four retained
legacy fixtures. The 306-line owning oracle is also included in the whole-owner count.

Legacy signatures/errors, basename grammar, opaque persisted bytes and cleanup/handle ordering remain
unchanged. No native caller, #331 persistence, canonical CAS or #348 reconciliation is activated.
Six contextual fixture groups are AUTHORED_NOT_COMPILED/NOT_RUN under #215; source adoption alone is
not native/runtime/product qualification. Production/static gates and final exact-head review remain
the manager's responsibility.
