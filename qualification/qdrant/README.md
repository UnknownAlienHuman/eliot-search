# Qdrant qualification

This directory is the P05–P07 evidence contract for the exact self-hosted Qdrant server/client pair
used by ELIOT Search.

- [`W3_QUALIFICATION.md`](W3_QUALIFICATION.md) — ownership, sequence, stop conditions and evidence.
- [`artifact.toml`](artifact.toml) — exact server/client/artifact identity; initially unqualified.
- [`collection-schema.toml`](collection-schema.toml) — architecture-required topology, vectors, payload
  fields and indexes.
- [`probes.toml`](probes.toml) — machine-readable mandatory capability probes.

These files select no product version by themselves. Indexed mode is disabled until immutable evidence
for one exact Windows x64 pair is independently accepted.
