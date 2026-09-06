# Publication guards: shared-field correction #141

Source baseline: `5c978e9f561dd502c7386dadffa288435568920c`.
Issue: https://github.com/UnknownAlienHuman/eliot-search/issues/141

## Correction

`PublicationIntent.owner_source_membership_access_guards` used a list of
`StateDependency`. Its kinds describe profiles/capabilities/auxiliary dependencies,
not the owner/source/membership/access/shadow/purge generations required by S13.4.
An empty list was accepted by the previous domain fixture.

The existing coordinator's exact seven-field `PublicationGuards` is now defined
once in `search-contracts.lifecycle`. `search_publication::PublicationGuards`
re-exports that same type. Intent constructors must supply the concrete guard value:
owner epoch, source catalog, membership, access, shadow and purge generations,
and the projection-profile digest. There is no default or list-to-guard conversion.

The field is documented inline in the existing P00 `PublicationIntent` schema.
It remains a nested immutable value under the existing `publication_records`
ownership group, not an additional durable schema root or mutable owner. No registry
schema count, package dependency, module ownership, port signature, state-machine
edge, journal format, hash algorithm, workflow or architecture policy is changed.

## Compatibility

This corrects the **shared Rust/P00 field shape** and breaks old intent struct
literals. The coordinator's field names, types, order, derives and root import path
are preserved. Existing binary journal headers, data records and receipts are not
rewritten. No deployed canonical intent codec is assumed to exist or to be compatible.

An external consumer with serialized list-shaped intents must use explicit version
rejection or a separately reviewed migration that obtains the missing authoritative
generations. It must not accept the list as complete guards, fill missing generations
with zeros, or relabel auxiliary digests. This change does not bump an unrelated
provider envelope version or fabricate accepted schema/API/evidence digests.

## Semantics that remain separate

The value carries expected observations; it is not a live owner guard, grant,
readback receipt or proof that counters cover the protected state. Their semantic
owners must advance those counters for all relevant changes. The control adapter
must compare actual values atomically at publication, validate exact manifests,
apply the matching shadows and publish the resulting snapshot before acknowledgement.

The full `ControlJournalPort`, H5 codecs and primary daemon migration are still
unfinished. This correction removes the unrepresentable-field blocker; it does not
claim those implementations, accepted T09/T27, or a qualified Qdrant pipeline.

## Verification

Added four shared-shape tests, two coordinator/shared-type identity tests, and two
domain regressions. The domain matrix exercises all 121 state pairs and checks that
the existing 23 allowed transitions preserve every prepared field. Two compile-fail
doctests reject the old dependency-list shape and default guard construction.

```sh
cargo +1.98.0 check --workspace --all-targets --all-features --locked
cargo +1.98.0 test --locked -p search-contracts -p search-domain -p search-publication --all-targets
cargo +1.98.0 test --locked -p search-contracts --doc
```

Rust compilation/tests are NOT_RUN in this authoring environment: Cargo is absent
and toolchain download hosts do not resolve. Source/diff/blob checks do not replace
execution. Issue #141 stays open pending executed checks and independent review;
no gate, lease, accepted handoff or qualification receipt is issued by this commit.
