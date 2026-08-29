# search-os-secrets

**Security support for C01, C15 and C30.**

**Status:** package boundary and agent contract only; behavior is intentionally unimplemented.

Own opaque OS-user/incarnation-bound secret references used by local process and provider bindings.

## Owns

- secret creation, resolve, rotate and delete lifecycle
- OS-user and installation-incarnation binding
- opaque `SecretRef` serialization
- plaintext exposure and side-channel prevention receipts

## Must not own

- Qdrant process lifecycle or provider session policy
- plaintext credentials in config, argv, logs or telemetry
- portable export of local secrets
- authority to grant source access

- **Delivery wave:** W1 / P01 foundation; qualified with P05/P14
- **Soft source-line target:** 3,500
- **Agent instructions:** [AGENTS.md](AGENTS.md)
