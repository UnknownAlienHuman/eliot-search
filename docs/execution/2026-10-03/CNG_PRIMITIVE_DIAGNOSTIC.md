# Windows CNG primitive diagnostic

**Result: observed primitive behavior; NON_AUTHORITATIVE.** The final retained attempt compiled
and exited 0 under Rust 1.98.0 on Windows x64. Independent Luna Max source/provenance review returned
`ACCEPTABLE_FOR_DIAGNOSTIC_PUBLICATION`. This is evidence toward the proposed native approval adapter;
it qualifies no profile, persistent key, actor, trust root, issuance operation or package.

The canonical bundle is [attempt-2](evidence/cng-primitive/attempt-2/result.json), source SHA-256
`E03C0285331382FED3FFDCAEB6DEDE557DCF416F71EB68CA0932A02EA2108132`.
Its [raw output](evidence/cng-primitive/attempt-2/probe.stdout.raw.json) records an unnamed temporary
ECDSA P-256 key in Microsoft Software Key Storage Provider, export-policy readback 0, a 72-byte
public blob, a 64-byte signature, successful verification through an imported public key, and
`NTE_BAD_SIGNATURE` (`0x80090006`) for changed digest and signature.

The null key name creates an ephemeral key, as specified by
[NCryptCreatePersistedKey](https://learn.microsoft.com/en-us/windows/win32/api/ncrypt/nf-ncrypt-ncryptcreatepersistedkey).
The private-export call requested only a size, with a null output buffer and zero output length;
it allocated or returned no private-key bytes. Its observed status was `0x80090029`
(`NTE_NOT_SUPPORTED`). The probe correctly reports `private_export_denial_verified=false`.
Microsoft documents Export Policy for persisted keys in its
[key-storage property identifiers](https://learn.microsoft.com/en-us/windows/win32/seccng/key-storage-property-identifiers);
this temporary-key observation does not prove the proposed persistent-key export restriction.

Same-provider verification and signature length do not independently qualify P1363 encoding or
interoperability. Persistence, actual profile qualification, pin/actor mapping, revocation and
operation recovery remain unverified. The [runtime inventory](evidence/cng-primitive/runtime-inventory.json)
identifies a System32 DLL on disk and explicitly leaves its actual loaded-module path unverified.

The [first failed attempt](evidence/cng-primitive/attempt-1/result.json) is retained unchanged:
compile exit 0, run exit 1, source SHA-256
`650F61BA8A3BA813D0349C1AABACFAB5CEDAD36B4C9AE752CA04F79E24C34BA8`.
It incorrectly asserted that a size query on an ephemeral key must prove export-policy denial.
The second source corrects that inference and reports the actual status without promotion to success.
The original first command names a top-level mutable source path; its exact historical input is
preserved in `attempt-1`. That top-level path now contains the second source while other top-level
files still describe the first attempt, so the mixed top-level snapshot is not a published bundle.

The original directory `C:\Development\Rust\targets\eliot-search-cng-diagnostic` is outside the nested
Search checkout but untracked inside the enclosing `C:\Development\Rust` repository. Each attempt's
source, raw streams and result JSON, plus the runtime inventory, were copied byte-for-byte into this
report's [capture manifest](evidence/cng-primitive/capture-manifest.json). Executables remain local;
their recorded sizes/hashes were checked against the retained binaries before copying. The source-base
metadata names `bac90ff7753ef7d4d107836c60480df216acfe7f`, the Search documentation base at capture,
not a claim that the diagnostic source was tracked in that commit. Git text conversion is disabled
for the evidence directory. No probe or compiler run was repeated during restart recovery or review.
