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
- legacy DIRECT revision compatibility uses exact inner-envelope bytes and an
  already-derived 32-byte optional-entropy value.

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
