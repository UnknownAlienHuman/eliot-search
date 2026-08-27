# tests

| Directory | Contents |
|---|---|
| `control-corpus/` | Acceptance corpus: an actively edited local function; renamed analogues across reference repositories; a same-name false positive; tests carrying a decisive edge case; mutually exclusive configuration variants; a fork and a mirror; a nested repository; stale, unindexed and inaccessible repositories; saved and unsaved edits; a watcher gap and resume; publication crash at each failpoint; access revocation during a query; a purge and restore attempt; a point-identity collision; multilingual documentation and non-ASCII paths. |
| `fixtures/` | Recorded deterministic fixtures: parser bundles, capability probes, golden lexical vectors, provider envelopes. |
| `property/` | Property and fault proofs over the invariants. |

Evaluation baselines are raw read and search, an existing comparison tool, and this system. A green
unit suite without acceptance evidence is not product acceptance.
