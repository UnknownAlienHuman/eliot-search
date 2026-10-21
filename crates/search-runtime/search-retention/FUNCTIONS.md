# Local object maintenance and current-format backup

The old lib/sweep/purge package implements leases, caller graphs, receipts and restore machinery. It is not
retained. Local object collection and backup are explicit stopped-root commands in existing store/control
composition. Qdrant cleanup remains separate live publication work.

## Stopped-root collection

Acquire the daemon's actual exclusive root guard. Busy means refuse, not kill its owner. Keep exclusion for
the command; there are no live producers, windows or old-view readers during the pass.

While serving, writers remove their own stages but do not collect final local objects. Disk pressure pauses
new writes and reports maintenance. This is an availability trade-off, not an online-GC claim.

The rejected online proposal traversed R reachable references before each of P deletion pages: potentially
O(P * R), despite a small deletion page. Do not repair that with another lease/reference-count framework.

1. Open one consistent current control snapshot. Enumerate current source/prepared refs, current-generation
   published source/evidence refs, pending inputs and unfinished cleanup refs.
2. Traverse only known typed reference metadata. Raw bodies are leaves; reachability does not require
   hashing every source.
3. Mark exact ObjectRefs once in disposable scratch redb data, with finite cache and bounded batches.
   Scratch is not product authority and must live outside the final object namespace.
4. Complete valid marking before any final deletion. Missing/corrupt/unsupported/truncated reference
   metadata, disk exhaustion or cancellation before sweep means zero final deletions.
5. Enumerate final objects once. Delete only recognized current-format unmarked identities through the
   object owner. Preserve foreign names, unknown formats and uncertain deletions.
6. Discard scratch after the run. Never resume an old mark set after interruption; rebuild under the next
   exclusive guard. Interruption during sweep can leave garbage, not select a live object for deletion.

Memory follows cache/page bounds; scratch disk grows with distinct references. Measure total traversal and
temporary disk, not just deletion-page size. Pending artifacts remain roots while Qdrant is unavailable;
maintenance does not execute or clear their effects. Stages are removable only as recognized store-owned
temporary objects. Source removal is not legal purge or a physical-erasure promise.

## Backup is an archive, not another runnable installation

The earlier #244 recipe copied current/pending/cleanup and then called normal startup. That would permit a
restored directory to replay work against the original Qdrant collection. A different local root lock does
not exclude the original installation from that external collection.

Create a coherent archive while the source is stopped and exclusively held. Close writers before copying
control/object files. Record format/storage requirements and any unresolved publication. Preserve current,
published, pending and cleanup references and the objects they need; never call a partial copy complete.

The supported backup command creates an archive marker/layout, not the daemon's launchable root manifest.
Normal open refuses that archive and performs no external effects. A raw OS clone of a runnable root is not
safe restore and cannot be assumed detectable merely from a new pathname. Do not promise automatic clone
detection or safe rollback from a copied process lock/UUID.

## Explicit restore/import

1. Keep the archive/original intact. Select an empty destination and the supported current format. Validate
   copied references and object integrity before making destination state usable. Failed import leaves an
   uncommitted destination, not a half-usable current root.
2. Create destination-local control state and independent installation/collection identity using ordinary
   initialization. Import committed source/prepared metadata needed for rebuild; do not adopt the copied
   current Qdrant route, pending apply cursor, cleanup rows or generation counters as executable authority.
3. Do not replay, compensate or delete anything named by copied pending/cleanup records. Keep them as archive
   evidence. The committed source inventory is the rebuild input, including actual gaps; it may differ from
   the last published inventory. If that inventory cannot be validated, refuse import rather than guess.
4. Use an explicitly selected destination endpoint and a fresh, non-conflicting physical Qdrant generation.
   An existing collection under the new name is a conflict, not an invitation to attach or delete. Rebuild
   through the ordinary publisher and install a destination view only after its real readback.
5. Destination cleanup can name only generations created/owned by this destination's normal publication.
   It never inherits permission to remove the archive's/original installation's collections.
6. The user retains the archive/original until verifying restored evidence. Restore does not delete the sole
   retained copy, run automatic GC over the archive or reread changed live files as a substitute for backup
   bytes. Incomplete source revisions remain explicit gaps.

This retains old Architecture 8.4 S28.1's useful new-installation/new-generation rebuild rule without its
paired-snapshot, quarantine, migration or receipt framework. Restart of the same installation and restore
of a copy are different operations; do not hide both behind one generic recovery entrypoint.

## Protected storage: readable is not writable continuity

Plain storage needs no key system. For a selected encrypted profile, same OS user/key availability proves
neither new nonce space nor safe writable restore. The public #598 contract uses a durable deterministic
AES-GCM counter and explicitly forbids copied/rolled-back counter continuity. Two writable copies with the
same actual key/counter could reuse nonces. A fresh root UUID or different AAD does not allocate a new key.

Never reset or resume copied encryption counters as fresh. Archived ciphertext may be read only if the
actual object reader supports its original key/AAD/source binding. New destination writes require the
accepted storage owner to establish an independent actual key/nonce context. If that profile cannot support
this, report writable restore as unsupported and keep the archive unchanged; do not silently downgrade,
relabel envelopes, re-encrypt or invent a generic key migration service. The private #598 implementation is
not inspected or modified by this recipe.

Primary algorithm reference: [RustCrypto AES-GCM 0.10.3 usage](https://docs.rs/aes-gcm/0.10.3/aes_gcm/#usage)
requires a unique nonce per message under a key. This is an algorithm constraint, not an argument for the
old six-domain/purge programme or a claim that #598 has shipped.

## Remove and verify

Delete search-retention, wave7-lifecycle, generic restore/purge/lease models and the proposed ObjectGcGate
with their final callers. Keep small maintenance/import behavior with real owners; no new restore service.

After code check busy-root refusal, complete mark coverage, malformed refs before deletion, scratch failure,
fresh marks after restart and foreign files. For backup/restore, keep original and destination concurrently
available at the same Qdrant endpoint: copied pending/cleanup must cause zero original mutations/deletes;
target generation must differ. Check interrupted import, missing objects/secrets, current-vs-published source
gaps and protected writable-copy refusal. These are required causal cases, not executed results.
