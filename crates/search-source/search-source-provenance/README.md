# search-source-provenance

The persistence/rehydration boundary for original v2 admission inputs under #569.
It retains complete normalized policy inputs once and references them from
observation/decision/receipt evidence. Reconstitution must call the existing
policy/observation/evaluation/receipt owners, never construct trusted receipt
fields from supplied fingerprints. Shared contracts own canonical encoding and
hash execution. This package performs no I/O and grants no native/root/access or
currentness authority.

Implementation and the actual Source registry consumer are work in progress.
This scaffold is not a merge candidate or product qualification.
