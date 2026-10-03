# Syntax-only qualified profile identifier implementation

**Integration source accepted:** `e88ab688d3b5cc6fb2562b5024448ed132a9a6da`.
Independent Luna Max source review of candidate `6749a6e31b080bd991accedeb13f5b187dd33a25`
returned `ACCEPTABLE_FOR_BOUNDED_INTEGRATION_PUBLICATION`. The integrated commit's three Git blobs
were compared to that exact candidate and matched. No product package or registry was changed.

The parser implements the separately [accepted syntax](../../handoff/changes/2026-10-03-qualified-profile-id-adoption.md):
one literal slash, 1–96 lowercase ASCII namespace bytes with single hyphen-separated alphanumeric
segments, 1–128 case-sensitive local `OpaqueId` bytes, and a maximum of 225 total bytes. It checks
the total bound before scanning and returns borrowed components without allocation or normalization.
The ASCII slash is a UTF-8 boundary, so component slicing is safe even for rejected Unicode input.
Errors are bounded codes, and scope is explicitly `NON_AUTHORITATIVE`.

The writer's one command was:

```text
cargo +1.98.0 test --locked --offline -p xtask --test qualified_profile_id
```

The original worker tool transcript reported **6 passed / 0 failed, exit 0**, at the exact candidate,
with clean source before/after; Rust/Cargo 1.98.0, Windows x64, UTC 09:42:29–09:42:38 on 2026-10-03.
The six tests exercise independent component and total limits, all accepted character classes,
malformed separators/segments, Unicode/controls/escapes, over-limit rejection and preservation of the
original borrowed bytes. Eleven warnings were reported in existing unrelated xtask modules.

**Evidence limitation:** the writer returned the process streams in its original tool transcript but
did not retain them as files. The [metadata](evidence/qualified-profile-id/verification-metadata.json)
and [result summary](evidence/qualified-profile-id/verification-result.json) are explicitly reconstructed
from that transcript; they are not original raw stream captures and make no stream-byte identity claim.
The [retention note](evidence/qualified-profile-id/stream-retention.txt) preserves this limitation.
The retained test executable matched its reported SHA-256, but its existence does not independently
prove the test result. No passing target was repeated merely to replace the missing stream files.

The four metadata/checksum files were copied with exact-byte readback; their
[capture manifest](evidence/qualified-profile-id/capture-manifest.json) records that proof's scope.
The binary stays local. Git text conversion is disabled for copied evidence. Source review and
integration-byte equivalence are independently verified; the test outcome is worker-transcript evidence.

Parsing creates no namespace ownership, profile registration, qualified store/signature, host pin,
actor mapping, lease or ticket. The canonical type-registry addition and the two field-rule consumers
still need their separate implementation/conformance; bootstrap and native issuance remain disabled.
