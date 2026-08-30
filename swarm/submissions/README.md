# Package submissions

Canonical layout:

```text
swarm/submissions/<package>/<submission_id>.toml
```

A submission binds one acknowledged, active ticket/lease/context chain; immutable base and final commits;
a complete sorted package-only diff; candidate public API/configuration identities; raw command outcomes;
unavailable checks; evidence; line budget and residual state. Configuration absence uses explicit
`OptionalV1` `ABSENT`, never TOML `null` or field omission.

A submission is independent-review input, not package acceptance, gate evidence or a wave receipt.

Use `swarm/SUBMISSION_TEMPLATE.md` and `swarm/schemas/package-submission-v1.toml`. This directory
currently contains no package submission.
