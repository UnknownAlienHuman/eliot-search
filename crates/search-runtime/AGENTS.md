# Runtime family rules

- `search-runtime-owner` owns the data-root owner epoch and process-wide lease only.
- `search-os-secrets` owns opaque OS-user/incarnation-bound secret references; plaintext never becomes
  public configuration, argv, logs or telemetry.
- `search-retention` owns CAS lifecycle, security/legal purge and restore quarantine.
- Ordinary retired Qdrant-point deletion belongs to `search-index-reclaimer`, not retention or purge.
- The daemon is the composition root; this directory itself is not a Cargo package.
- Runtime packages never decide retrieval meaning or client admission.

Do not add a family-level `Cargo.toml` or shared implementation. Put behavior in the child package that
owns the failure state and test seam.
