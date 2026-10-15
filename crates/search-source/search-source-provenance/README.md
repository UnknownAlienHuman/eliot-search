# search-source-provenance

The persistence/rehydration boundary for original v2 admission inputs under #569.
It retains complete normalized policy inputs once and references them from
observation/decision/receipt evidence. Reconstitution must call the existing
policy/observation/evaluation/receipt owners, never construct trusted receipt
fields from supplied fingerprints. Shared contracts own canonical encoding and
hash execution. This package performs no I/O and grants no native/root/access or
currentness authority.

The semantic Source registry and normal metadata restoration consume these
originals. Acceptance remains tracked by #569; Native ingestion and indexed
product qualification remain with their owning integration issues.
