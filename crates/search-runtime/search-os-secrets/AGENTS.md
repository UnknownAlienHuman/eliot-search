# Working on search-os-secrets

Read the [runtime direction](../README.md), [package audit](FUNCTIONS.md), package [README](README.md),
[implementation guide](../../../docs/implementation/README.md) and
[storage-profile note](../../../docs/implementation/revision-encryption.md).

Do not extend or repair the generic `SecretCatalog`, leases, rotation/revocation, operation receipts,
configuration fingerprints or provider-neutral backend model. #307/#308/#309 are closed `not_planned`.
Historical size/caller findings are revision-specific. Before deleting or moving this package, recheck current
manifests, Rust/non-Rust callers, constructors, features, targets, tests and workflows, then change all actual
consumers in the same cut.

Concrete ownership:

- Qdrant owns one optional actual key in its composition;
- trusted local protocol has no pairing secret, grant or per-frame MAC;
- a restricted-client adapter owns its selected credential/session;
- encrypted immutable storage owns one concrete OS-protected key path and object codec;
- plain immutable storage has no secret.

Preserve #598's active private work and consume only its accepted handoff. Do not implement a competing revision
key/object writer or infer its final format. After handoff, retain only exact metadata/helpers directly used by
the selected current profile and delete unconsumed candidate/selection/lifecycle code.

Keep an old-format reader only while explicit user-data access/export needs it. It must not create new legacy
objects, migrate on open or constrain the current format. Change direct daemon/config/protocol/object consumers
together; do not add a V2 secret platform or forwarding facade.

After code, run minimal Rust 1.98 check/Clippy for actual retained profile and direct consumers, then focused
fresh/existing/restart/wrong-key/corruption/no-secret-output cases. Historical catalogue/rotation/grant matrices
are removed with their APIs.
