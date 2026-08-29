# W7 hardening — `search-index-reclaimer`

This packet makes the boundary between ordinary retired-point reclamation and security/legal purge
machine-reviewable.

## Accepted authority

The ordinary reclaimer accepts only:

```text
committed normal publication receipt
exact retired-point manifest emitted by that commit
current route/epoch watermark
ordinary_reclaim operation class
```

It rejects purge requests, purge fences/tombstones, restore manifests and generic delete filters. Its
receipt is never a required purge-layer receipt.

## Purge overlap

If a purge fence overlaps an ordinary reclaim plan, the plan pauses and revalidates. The lifecycle purge
path may independently delete the same exact physical point under a different operation/receipt class.
Ordinary resume exact-readbacks IDs and accepts already-absent only as ordinary completion when the
identity/generation matches; it never imports the purge receipt or claims purge completion.

## Restore interaction

Objects/points in a restore quarantine generation are not ordinary retired points and cannot enter the
reclaimer until a new serving route/publication later emits a committed retirement manifest. Old backup
retirement metadata is not trusted.

## Required tests

- purge command/fence/receipt rejected as ordinary reclaim authority;
- ordinary receipt fails purge receipt type/verification;
- overlap race: purge deletes first, ordinary exact readback completes only ordinary plan;
- ordinary delete first does not satisfy logical/handle/CAS/backup purge layers;
- restore quarantine generation never reclaimed from old backup metadata;
- broad delete/filter API remains absent;
- receipt always states `security_purge = not_claimed` and `physical_secure_erase = not_claimed`.
