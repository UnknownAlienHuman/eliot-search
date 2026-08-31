# Local context artifact candidates

This directory is reserved for ordinary local output from
`tools/build-context-artifact-candidate.py`.

A generated `.context` file contains deterministic `ELIOT_SWARM_CONTEXT_1` bytes. Its adjacent `.json`
file records exact source, registry-fragment and prerequisite-handoff identities. Neither file is an
immutable artifact reference, a committed `context_manifest_v1`, an assignment ticket, a lease, accepted
evidence or implementation authority.

Generated files are ignored. Only this metadata and `.gitignore` are committed.
