# search-os-secrets

**Security support for C01, C15 and C30.**

**Status:** package boundary and agent contract only; behavior is intentionally unimplemented.

Own opaque OS-user and installation-incarnation-bound secret references for local process and provider bindings.

## Owns

- secret creation, guarded use, rotation and deletion lifecycle
- OS-user, installation and purpose binding
- opaque references and non-secret receipts
- plaintext side-channel prevention

## Must not own

- Qdrant process lifecycle or provider authorization
- plaintext secrets in public APIs, config, argv, logs or telemetry
- portable export of local credentials
- source-access authority

- **Delivery wave:** W1 / P01 foundation; qualification continues in P05/P14
- **Soft source-line target:** 3,500
- **Agent instructions:** [AGENTS.md](AGENTS.md)
