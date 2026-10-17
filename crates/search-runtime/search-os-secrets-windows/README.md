# search-os-secrets-windows

Concrete Windows current-user secret effect adapter.

The package owns the native boundaries for:

- Credential Manager read/create/readback of the 32-byte legacy revision root
  secret;
- `BCryptGenRandom` generation of a missing root secret;
- the named cross-process revision-vault mutex;
- `CryptProtectData` / `CryptUnprotectData`;
- Credential Manager and DPAPI native allocation lifetimes;
- bounded copy-out and clear-before-free.

It performs no filesystem, registry, source-catalog, process, or policy
I/O. Daemon composition scans the admitted revision root and passes exactly one
requirement: create is allowed only when no protected revision object exists;
otherwise the original credential is mandatory.

The create path rechecks Credential Manager after acquiring the named mutex. A
concurrent process that published a key after the initial read therefore wins;
a stale generated candidate never overwrites that key. Every successful write
must read back the exact same 32 bytes before the secret is returned.

All revision create functions use the same kernel and attempt at most one
credential write per invocation. Lock/read retries remain finite before that
write. Once publication is attempted, any write failure, absent/invalid/mismatched
readback or context refusal returns `CredentialWriteOutcomeUnknown`; the invocation
never retries publication, generates a replacement key or deletes a credential.

The additive `*_with_context` functions borrow the original `AtomicBool` and
receive the original absolute `Instant`. They poll before and after native reads,
RNG, locks, writes/readback and sleeps, and before returning a secret. Retry sleeps
consume only remaining original time. Existing-only reads never generate or write.
`LegacyRevisionRootSecretContextError` preserves existing exhaustive legacy error
consumers and exposes stable content-free Debug/Display codes. Legacy APIs remain
context-free until their actual consumers cut over.

These checks are cooperative: the existing native mutex seam can wait five
seconds and other synchronous native calls are not forcibly preempted. Expiry or
cancellation refuses subsequent work when a seam returns; this does not promise
hard response-time bounds or native qualification. Fixtures are authored with
their owning source and execute during integrated qualification under #215.

Three production surfaces are available:

- legacy revision root-secret operations use `LegacyRevisionRootSecret`,
  `LegacyRevisionRootSecretRequirement`, and the two load functions;
- short application secrets use `SecretBytes`, `ProtectionScope`, and
  `ProtectedSecret`;
- the selected Qdrant API key uses read-only `QdrantApiSecretStore`, a fixed
  Qdrant-purpose reference and finite `QdrantApiSecretLease`; no credential
  provisioning or rotation is exposed by this adapter;
- legacy DIRECT revision compatibility uses exact inner-envelope bytes and an
  already-derived 32-byte optional-entropy value.

The Qdrant lease expires no later than the remaining issuer operation deadline
and retains that operation's cancellation capability and request coordinates.
Disclosure requires the original context identity and the same native monotonic
lease window before and after the callback; a fresh context cannot extend or
replace either bound. The credential record's version is included in Qdrant-only
DPAPI entropy while its Credential Manager locator stays stable, so a changed
header version cannot reuse the old protected payload. No detached launch
evidence is exposed; `with_secret_live` supplies the exact frame digest only
inside the callback so Root can compare its C02 inventory and dispatch there.

The optional `test-credential-cleanup` feature adds one exact-namespace cleanup
operation for native test harnesses. It serializes with production key creation,
uses the same package-owned target grammar and vault mutex, and returns success
only after Credential Manager readback proves absence. Production daemon builds
do not enable this feature.

Secret owners are non-clone, redact `Debug`, and overwrite their Rust buffers on
drop. Native Credential Manager and decrypted DPAPI allocations are cleared
before release when their reported length is within the admitted bound.
Oversized invalid native output is released without an unbounded memory clear.

The pure envelope, root-secret shape, and key-derivation contracts remain in
`search-os-secrets`. The daemon retains source/catalog authority, protected-object
inventory, concrete SHA-256, persistence, namespace-file parsing, and translation
to historical `DIRECT_*` compatibility reasons.

## Original-context legacy DPAPI port

The additive legacy DPAPI *_with_context functions require one mutable
LegacyRevisionDpapiContext. The caller retains its original deadline, flag,
cumulative capacity/processing/native-call meter and sticky first refusal.
No default contextual callbacks, fresh budget, credential operation, filesystem
effect, new crypto profile or envelope authority is supplied by this port.

Existing short-secret and legacy revision APIs keep their signatures and errors
and execute the same private native/copy kernel through the compatibility context.
Their current-user scope, 32-byte legacy entropy and 65MiB revision ceiling remain
unchanged. The pure inner/outer envelope contract stays with search-os-secrets;
its 148/156-byte framing and selected-key bindings are not edited here.

Before dispatch, the kernel reserves all possible native attempts (Crypt32,
GetLastError, output LocalFree and, for unprotect, description LocalFree). Entropy capacity/copy/clear is charged
before its Rust allocation. After validating the returned native pointer/length,
the kernel charges that exact accepted output capacity and copy/possible Rust
clear before allocation. No 65MiB output capacity reservation or post-return
capacity debit is used. Requested capacity stays cumulative with no refund.

NativeOutput stays owned until an explicit contextual finish on every ordinary
Result path. Before its bounded volatile clear, mandatory account_native_clear
records the actual attempted bytes in the SAME cumulative processing meter,
even after sticky refusal. Overflow/exhaustion evidence and first refusal stay
retained. The callback cannot veto clearing/freeing an already-owned buffer.
Mandatory bounded cleanup may exceed remaining allowance after unavoidable
native return; it records attempted work without raising the allowance or
permitting further ordinary work. Success requires the original context to
remain within its allowance after cleanup. This is not a hard executed-cleanup,
CPU/RSS, physical erasure, preemption or throughput guarantee. Drop remains
resource-safety fallback for unwinding; no unwind accounting guarantee is made.
Every native/description/entropy guard is released before the final original
checkpoint. Native/allocation and post-refusal causes remain typed; Debug and
Display redact context, allocator detail and numeric native errors.
Ordinary cleanup releases output, description, then entropy. Failed-unprotect
cleanup order can differ from the former implicit stack-drop order; legacy
error codes and successful returned bytes remain compatible.
Rejected oversized native buffers retain existing free-without-unbounded-clear
behavior. OS-internal allocations happen before their sizes are known: neither
this port nor the cooperative caller predicts/preempts them.

The daemon consumer successor must precharge its actual inner (P+148), ciphertext
clone, outer (C+156), verified plaintext and entropy construction at their owning
seams, then use these contextual functions for BOTH protect and unprotect.
This owner API does not activate native sync or eliminate its legacy caller.
The preserved full-sync proposal stays separate, incomplete and unadopted.

Seven private fixture groups are AUTHORED_NOT_COMPILED/NOT_RUN, including the
Windows round trip. The full retained owner and fixture source is counted in
the #471 scope; earlier credential or daemon exceptions grant no new scope.
Native/runtime/product qualification remains #215. Source presence is not
qualification.
