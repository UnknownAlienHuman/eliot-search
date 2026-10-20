# Storage protection and concrete credentials

The old `[secrets]` section attempted to configure one universal backend, lease TTL, rotation grace and
Qdrant secret reference. The current product has no generic secret service. Configure each concrete consumer
at its owner.

## Storage protection

A fresh current-format root selects one immutable storage profile at initialization:

```toml
[storage]
protection = "plain"                 # or the accepted encrypted profile
```

The final encrypted profile name/fields are adopted only from #598's accepted handoff. Until then, do not
invent another key/object writer.

Rules:

- the profile is root/storage identity and is not changed live;
- profile mismatch refuses before writes;
- `plain` uses the one immutable object format with no key;
- an encrypted profile uses one concrete Windows-user key owner and authenticated object codec;
- already encrypted pre-release data is handled by an explicitly retained reader/export/rebuild path or the
  root remains unsupported and untouched;
- no generic key lease TTL, rotation grace, current/candidate roles or cross-root migration configuration.

## Qdrant credential

Qdrant's optional actual API key belongs to the `[qdrant]` configuration/secret-loading owner documented in
[`qdrant_process.md`](qdrant_process.md). Rebuild the Qdrant client when it changes. Do not route it through a
universal `SecretRef` catalog.

## Restricted-client credential

An optional restricted-client adapter owns its selected authentication reference, allowed scope, session
expiry and revocation. Trusted child stdio/owner-only local query has no application pairing key, grant or
per-frame MAC.

## Parsing and application

Use ordinary bounded typed configuration:

```text
parse selected concrete fields
→ validate selected profile/consumer
→ restart/reopen that owner when static values change
```

Do not create section descriptors/fingerprints, generic reload plans, application receipts or a secret
lifecycle operation journal. Plaintext key values must not enter repository files, argv, logs or diagnostics;
a selected local secret source may provide the concrete Qdrant/restricted/storage credential directly to its
owner.

## Diagnostics

Expose only useful redacted state:

- selected storage profile;
- key available/unavailable for the current encrypted root, without value/reference detail;
- Qdrant authentication enabled/disabled and connection result;
- restricted-client profile enabled/disabled.

No raw credential, DPAPI blob, SID, absolute private path, source/query bytes, binding digest or lifecycle
receipt is shown.

## Focused checks

- unknown profile/field rejected;
- profile mismatch refuses before writes;
- plain profile performs no credential/DPAPI work;
- encrypted profile existing-key absence/wrong binding fails without replacement generation;
- concrete credential values remain absent from argv/logs/debug/config serialization;
- static profile/credential changes require restart of their actual owner.
