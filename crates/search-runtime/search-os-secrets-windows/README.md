# search-os-secrets-windows — keep only selected Windows effects

Concrete Windows effects are useful only when a selected current consumer uses them. This package is not a
general credential/key lifecycle for Qdrant, provider pairing, storage, backup and migration.

Read the [runtime direction](../README.md), [core secret audit](../search-os-secrets/FUNCTIONS.md), local
[source map](FUNCTIONS.md) and [storage-profile note](../../../docs/implementation/revision-encryption.md).

## Verified public-main inventory

Production source is about **133.1 KiB before tests**, plus an optional ~7.5 KiB test-credential cleanup
surface:

```text
credential.rs                    15.3 KiB
credential/context.rs             5.6 KiB
credential/pairing.rs            12.9 KiB
credential/revision_dek.rs       31.4 KiB
credential/windows.rs            20.3 KiB
dpapi.rs                         20.8 KiB
dpapi/context.rs                 10.5 KiB
lib.rs                            2.4 KiB
model.rs                          9.9 KiB
revision_selection.rs             4.1 KiB
```

Current consumers span unrelated designs:

- legacy DIRECT root-secret and DPAPI revision protection;
- revision-DEK candidate/current selection scaffolding;
- legacy migration/context work meters;
- provider pairing credentials for the old paired protocol;
- CLI/native binding credential helpers;
- storage ACL/owner composition.

The first three intersect #598/legacy data. Provider pairing is removed by #280/#332. Universal backend and
revision-key lifecycle issues #307/#308/#309 are closed `not_planned`.

## Target

After #598's accepted handoff, keep only the concrete Windows operations selected by the current encrypted
storage profile, for example:

```text
read existing exact current-user protected key/reference
create once for an explicitly new encrypted root when policy allows
exact Credential Manager/DPAPI readback
DPAPI protect/unprotect of the small selected key material
actual SID/user/root binding validation needed by that profile
bounded native allocation ownership and redacted errors
```

The immutable-object owner determines when key creation/use is valid and owns object identity/persistence.
This package performs only the Windows call and returns actual readback/material through a narrow non-public
owner.

Plain storage invokes none of this.

## Remove with callers

- provider-pairing credential intent/read/create/delete and its CLI/daemon constructors;
- generic `SecretReference`/purpose/lease/catalog integration;
- revision-DEK candidate/current selection metadata not consumed by #598's accepted profile;
- migration-only context/work meters and compatibility adapters after legacy migration is cancelled;
- old full-sync/work-accounting protocol which exists only to preserve previous caller shapes;
- generic short-secret API if no selected restricted consumer remains;
- test cleanup/credential namespaces and fixtures for removed profiles;
- duplicate legacy/current wrappers around the same CredMan/DPAPI call.

Do not add a provider trait, generic Windows vault, current/candidate key store or a new receipt protocol.

## Legacy old-format support

A narrow read-only legacy profile may retain:

- exact existing root-secret read; never generate a replacement when protected old objects exist;
- legacy DPAPI unprotect with exact entropy/binding;
- returned plaintext validation in the legacy object reader.

Do not create new old-format objects or automatically migrate them on ordinary open. Unsupported old roots may
be refused before writes and left intact.

## Required Windows semantics

- hold and free native allocations correctly;
- reject invalid/oversized native lengths before Rust allocation/copy;
- keep raw credential/key/plaintext out of Debug/Display/logs/argv;
- best-effort clear owned Rust/native buffers without secure-erasure claims;
- exact read-after-write when creating the one selected key;
- existing-data key absence/mismatch is a hard failure, not replacement generation;
- cancellation/deadline checks are cooperative around synchronous OS calls and are not hard real-time claims;
- uncertain credential write resolves by reading the exact target, not a generic operation history.

#598 remains active private work. Do not modify or duplicate its unpublished writer/key-use design. This audit
only removes unrelated public-main lifecycle/platform obligations around the accepted handoff.
