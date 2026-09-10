# Qdrant native Windows install (no Docker)

Qdrant runs natively on Windows x64. No Docker, no WSL, no Linux VM.

## Installed set (pinned, measured 2026-09-10)

- version: `1.19.0` (build `74f3e85b`), `x86_64-pc-windows-msvc`
- archive: `qdrant-x86_64-pc-windows-msvc.zip`, 29340688 bytes,
  SHA-256 `980CB2E1AE771155CF211DA8C0A8A9206B6482BD4EFFDC4DB994D3ADB707B087`
  (self-measured over TLS from the official release URL; upstream publishes
  no checksum file, so there is nothing to cross-check against — the bytes
  above are the recorded identity)
- binary: `C:\Tools\Qdrant\1.19.0\qdrant.exe`, 84184576 bytes,
  SHA-256 `369C562EAE3D89333A13ABFDB522FA209E3F587C1217A1059D817E80814EA9D4`
- license: Apache-2.0
- receipt: `C:\Tools\Qdrant\1.19.0\VERSION.txt`
- smoke 2026-09-10: `READY ok / CREATE ok / UPSERT ok / SEARCH top_id=1 /
  COUNT=2 / DELETE ok` (`C:\Tools\Qdrant\tools\qdrant-smoke.ps1`)

Layout rule: one versioned directory per release (`C:\Tools\Qdrant\<x.y.z>`).
Data directories always live outside the install dir (passed per-run).

## Run (manual / supervisor-owned)

Qdrant takes a config file, not storage/port flags:

```powershell
pwsh -NoProfile -File C:\Tools\Qdrant\tools\qdrant-smoke.ps1 -QdrantDir C:\Tools\Qdrant\1.19.0
```

Minimal `config.yaml`:

```yaml
storage:
  storage_path: ./storage
service:
  http_port: 6333
  grpc_port: 6334
```

Product execution (loopback-only, API-key lease, ACL, Job Object, owner
guard) belongs to `search-qdrant-supervisor` (T23), not to this document.

## Updates (manual pinned procedure, never automatic)

Invariant 17 forbids automatic upgrade and silent provider switching. Every
update is an explicit, pinned, re-qualified event:

```powershell
pwsh -NoProfile -File C:\Tools\Qdrant\Update-Qdrant.ps1 `
  -Version <X.Y.Z> -ExpectedZipBytes <N> [-ExpectedZipSha256 <hex>]
```

The script installs side-by-side, verifies size (+ SHA-256 when supplied),
smoke-tests, and stops. It never switches live traffic, never deletes old
versions, and never qualifies anything. After it:

1. write the `VERSION.txt` receipt in the new version dir;
2. re-run the smoke against the new dir;
3. re-execute the W3 qualification order (`qualification/qdrant/`) —
   product use of the new version is forbidden until `artifact.toml`
   reaches `QUALIFIED` with fresh reviewer receipts;
4. remove the old version dir only after the new one is qualified
   (storage must be migrated or rebuilt per the migration receipt, never
   blindly reused across versions).

One minor version at a time (upstream rule); consult the upstream changelog
for storage-format notes before touching data.
