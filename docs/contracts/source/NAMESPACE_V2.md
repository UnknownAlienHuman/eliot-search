# Permanent-v2 namespace-owner generation — #569

This pure profile supplies the identity axis required by Architecture S7.2.1.
It does not allocate a namespace, admit an owner or verify live custody.
`SourceOwnerScopeV2.epoch` is the admitted namespace ownership epoch.
`RootRecord.owner_epoch` retains the historical Native registration fence;
physical Native/journal custody may advance without rewriting that Root value.
Source preflight checks active Root/revision/policy and namespace ownership as
separate domains. For an existing namespace its provider, incarnation, epoch and
generation must still match exactly. Fresh Native custody is mandatory in the
normal adapter; no equality of metadata scalars proves it or authorizes ownership
succession. A physical journal advance alone never changes a namespace owner.
The existing `search-domain` owner computes it with the sole shared canonical
encoder and BLAKE3 executor; no new package or dependency is introduced.

The complete preimage is exactly:

```text
UTF8("eliot/cbor/source-owner-generation/v2") || 00 || deterministic-CBOR([
  2,
  source_namespace_id: bstr16,
  owner_system_id: exact bounded UTF8 text,
  owner_installation_incarnation_id: bstr16,
  owner_epoch: nonzero uint64,
  ownership_record_revision: nonzero uint64,
  status: "ACTIVE" | "CUTOVER_PREPARED" | "FENCED" | "RETIRED"
])
```

The CBOR profile is `eliot.cbor.length-first.v1`. The result is the full
32-byte BLAKE3 digest. The complete preimage ceiling is 512 bytes and admits
the contracts owner's full 256-byte owner identifier. The maximum typed
preimage is 368 bytes, including uint64 maxima and `CUTOVER_PREPARED`.
The supplied generation,
admission-policy revision and cutover reference are excluded: policy has its own
axis, while a cutover changes the bound status/revision/owner fields.

For namespace `[01;16]`, owner `search-provider`, incarnation `[02;16]`, epoch 3,
revision 4 and `ACTIVE`, the 61-byte CBOR payload is:

```text
870250010101010101010101010101010101016f7365617263682d70726f76696465725002020202020202020202020202020202030466414354495645
```

The complete preimage is 99 bytes; BLAKE3 is
`88af571a68fd22a2f870587ce1a8c3ec0d98bfab3238721430749ce115d3819b`.
The golden was independently hashed from those manual CBOR bytes using the
pinned BLAKE3 artifact, without the Search encoder or domain implementation.

The current-v2 registry initializer must take actual qualified namespace and
installation identity from normal owner composition, verify its predecessor,
Root/policy/owner/security fences and persist the exact ownership in the same
versioned registry state/history. The daemon must retain the original operation
checkpoints before and after this bounded pure computation. A computed digest
alone never proves namespace allocation, current ownership or durable readback.
The semantic initializer now creates Source/namespace metadata atomically through
the existing registry control port. Normal daemon metadata restoration consumes
the explicit v4 profiles and original inputs with the caller's checkpoints,
including retained legacy members. Native ingestion/caller qualification remains
with #267/#270; full acceptance of this slice is still tracked by #569.

The v2 domain is explicit. Retained v1 profiles, arbitrary historical generation
values and old operation bytes are not rewritten, relabelled or implicitly
upgraded. New state/history formats must select this profile explicitly.
