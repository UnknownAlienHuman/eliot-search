# Working on search-os-secrets

Read the [runtime direction](../README.md), [package audit](FUNCTIONS.md),
[implementation guide](../../../docs/implementation/README.md) and
[storage-profile note](../../../docs/implementation/revision-encryption.md).

Do not extend or repair the generic `SecretCatalog`, leases, rotation/revocation, operation receipts,
configuration fingerprints or provider-neutral backend model. #307/#308/#309 are closed `not_planned`.

Concrete ownership:

- Qdrant owns one optional actual key in its composition;
- trusted local protocol has no pairing secret/grant/MAC;
- a restricted-client adapter owns its selected credential/session;
- encrypted immutable storage owns one concrete OS-protected key path and object codec;
- plain immutable storage has no secret.

Preserve #598's active private work and consume only its accepted handoff. Do not implement a competing
revision key/object writer or infer its final format. After handoff, retain only the exact metadata/helpers it
uses and delete unconsumed candidate/selection/legacy lifecycle code.

Keep an old DIRECT reader only while explicit user data access/export needs it. It must not create new legacy
objects, migrate on open or constrain the current format.

Change direct daemon/config/protocol/object consumers together. Delete pairing `secret_composition`, generic
config/lifecycle and fixtures with their last callers; do not add a V2 secret platform or forwarding facade.

After code, run minimal Rust 1.98 compiler/Clippy for actual retained profile/consumer targets and focused
fresh/existing/restart/wrong-key/corruption/no-leak cases. Historical catalog/rotation/grant matrices are not
gates.
