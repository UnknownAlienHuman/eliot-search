# Windows secret effects — source audit and replacement

The package owns native Windows calls, not secret policy or lifecycle. Its public surface must collapse to
the exact effects used by the selected storage/restricted-client profile.

## Current source/caller map

| Area | Current use / disposition |
|---|---|
| `credential/pairing.rs` and related credential APIs | Used by old provider pairing/native-binding paths. Delete with #280/#332 unless a separately selected restricted client retains a concrete credential. |
| legacy revision root-secret APIs | Used by DIRECT protected-object open/read/write. Keep only as a narrow old-format reader while explicit user data needs it; do not create new legacy objects in the current format. |
| `credential/revision_dek.rs` + `revision_selection.rs` | Used by owner-composition candidate/current scaffolding. Keep only fields/effects selected by #598's accepted current encrypted profile; delete the remaining candidate/selection ceremony. |
| DPAPI short-secret APIs | Retain only if a concrete current consumer exists; Qdrant and trusted local protocol do not need the generic wrapper. |
| DPAPI legacy-revision contextual API | Large cumulative budget/native-call/cleanup accounting layer for legacy migration/callers. Remove after those callers are retired; do not port it into the current object format. |
| `test_credential_cleanup` | Feature-only fixture cleanup. Delete with retired credential namespaces/profiles. |

## Target Windows key adapter

Concrete shape after #598 handoff should be no broader than the accepted profile requires, for example:

```text
read_storage_key(target, expected_user/root/profile, context)
    -> owned redacted key | Missing | BindingMismatch

create_storage_key_once(target, expected_user/root/profile, context)
    -> owned redacted key

protect_small_key_current_user(plaintext_key, entropy, context)
    -> protected bytes

unprotect_small_key_current_user(protected, entropy, context)
    -> owned redacted key
```

This is illustrative behavior, not a requirement to create these exact functions. Use existing platform code
where it is correct, but change direct callers and delete obsolete wrappers together.

Rules:

1. Existing encrypted data never triggers generation of a replacement key.
2. Key creation is allowed only for an explicitly new root/profile with no protected objects/current key.
3. Credential creation serializes on the exact target and rechecks before write; a concurrent existing key
   wins.
4. After a possible write, read the exact target and compare actual bytes. Unknown outcome is resolved there.
5. Key bytes are owned, non-clone where practical, redacted and finite. They are not wrapped in a public
   lease/receipt or stored in redb.
6. DPAPI entropy/AAD comes from the concrete selected object/key profile, not a generic provider purpose.
7. Native allocations are bounded/validated/freed; cleanup accounting is internal, not a public authority.
8. Plain storage and Qdrant endpoint mode never call this adapter.

## Pairing and generic credential removal

Delete:

- `ProviderPairingCredentialIntent`, purpose/generation/reference grammar and CLI reader;
- pairing key create/read/delete/readback and daemon `PairingSecretComposer`/vault/rotation/revocation;
- application pairing/native credential requirements for owned child stdio and owner-only pipe;
- generic Qdrant credential reference/lease integration;
- profile/capability/currentness receipts attached to credential access.

A real restricted-client adapter, if selected, defines its own credential and session lifecycle and may reuse
a small underlying Windows read/write primitive without exposing this old protocol.

## Revision-key selection removal

After the accepted #598 handoff is public:

1. trace the exact key/reference bytes and Windows calls it actually uses;
2. move only those operations to the immutable-object/key owner boundary;
3. remove unconsumed candidate, intent, current-selection and control-record codecs;
4. remove multiple historical epoch/version/fingerprint axes which no stored current record uses;
5. remove `search-revision-crypto` if it remains unreferenced;
6. remove legacy migration/context adapters and compatibility fixtures.

Do not alter #598's active private branch/worktrees or assign a competing implementation.

## Failure behavior

- unsupported platform/profile: explicit unsupported error;
- wrong current user/SID/root/profile/entropy: binding error;
- missing existing key: unavailable/corrupt root, never fresh generation;
- invalid native pointer/length: platform failure, bounded release;
- timeout/cancel before mutation: known failure;
- timeout/cancel after possible credential write: exact target readback decides known/unknown;
- DPAPI decrypt failure: no plaintext and no fallback;
- delete means future target absence only, never secure erase/backup purge.

## Focused checks

Keep tests only for retained direct effects: new-root create/readback, existing read, concurrency race, wrong
user/binding, malformed/oversized native result, DPAPI round trip/tamper, redacted diagnostics and unknown
write readback. Delete pairing/generic lease/migration/current-selection matrices with their APIs.
