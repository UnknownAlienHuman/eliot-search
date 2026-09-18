# search-os-secrets

**Security support for C01, C15 and C30.**

Owns the pure finite lifecycle for opaque, user/installation/incarnation/purpose-bound local secrets.
The package performs no platform I/O and returns plaintext only through a bounded non-clone lease.
Qualified Windows effects remain in `search-os-secrets-windows` and daemon composition.

The package also owns the frozen pure byte-layout, binding-validation and key-derivation transcript for
the legacy DIRECT protected-revision envelope. The compatibility contract fixes the key-binding and
DPAPI-entropy domains plus namespace/root-secret part ordering, but performs no digest, DPAPI,
Credential Manager, filesystem, source-catalog or plaintext-admission operation. Concrete hashing and
platform effects remain injected by daemon composition.

## Owns

- encrypted-record creation, guarded use, rotation and deletion lifecycle
- OS-user, installation, incarnation and purpose binding
- opaque references and content-free receipts
- plaintext side-channel prevention
- legacy revision outer/inner envelope framing and exact binding validation
- legacy revision platform-key and DPAPI-entropy derivation transcripts

## Must not own

- Qdrant process lifecycle or provider authorization
- Credential Manager, DPAPI, RNG, concrete hashing, filesystem or process effects
- plaintext secrets in public serialization, config, argv, logs or telemetry
- portable export of local credentials
- source identity, source-access authority, catalog policy or revision-store persistence

- **Delivery wave:** W1 / P01 foundation; qualification continues in P05/P14
- **Soft source-line target:** 3,500
- **Agent instructions:** [AGENTS.md](AGENTS.md)
