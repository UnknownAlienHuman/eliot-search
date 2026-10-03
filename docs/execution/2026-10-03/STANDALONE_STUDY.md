# Standalone repository study

Status: **BASELINE_PREPARED; ELIOT_SEARCH_UNAVAILABLE**. The user authorized this experiment and required
standalone operation without Eliot Governor or Memory OS. Luna Max performed the repository study;
ordinary-search findings are not Eliot Search results or product acceptance.

Corpus: `C:\Development\Rust\projects\eliot-swarm-controller`. Initial immutable Git base:
`3ecdf52707731e3f85e85827a88fbdb28d784f3e`. The baseline used current working-tree bytes, including
pre-existing modifications and untracked files. All twelve selected anchor files had explicit SHA-256
captures. Six existed as blobs at the pinned base; six were explicitly absent there. An absent blob was
not replaced by a moving `HEAD` lookup.

The corpus changed concurrently to observed commit `9dc5caf7298486470b944e2e8d5b8998075b3b46`.
All twelve selected working-tree byte hashes still matched on the final bounded integrity check.
Git/status deltas are recorded separately; no stable whole-repository snapshot or clean corpus is
claimed. The study agent made no corpus writes.

| ID | Exact query prepared for both search methods | Question topic |
| --- | --- | --- |
| Q1 | `allows_method` | Tool-profile restrictions |
| Q2 | `latest_due_slot` | Scheduler catch-up and repeated admission |
| Q3 | `cancel_requested` | Cancellation and owned child-process completion |
| Q4 | `prepare_push` | Safe publication and remote-ref readback |
| Q5 | `owner_policy` | Attempt-policy binding |

At the availability observation on Search source `99db410fc540104928284f2ecd206b4b5baec8a0`, neither
source-matched CLI nor daemon existed in the configured Cargo target, local target directories or Cargo
bin directory, and neither command was on PATH. No daemon was started, corpus admitted or indexed, or
Eliot Search query executed. Later integration-only source commits did not trigger a repeated unchanged
product build. There is no measured Search recall, ranking, latency or quality comparison.

The CLI source defines DIRECT commands `serve-data-root`, `index-directory` and `search-root`, including
a trailing `--daemon PATH` option. Example configuration selects standalone mode; W8 documents that the
ELIOT profile is optional and unnecessary for standalone G4. Source wiring is not proof of a working
runtime. Qdrant support is optional under `wave3-index` and was not enabled for this baseline study.

The detailed report, exact-term and broader-search baselines, corrected manifest, initial/after status,
anchor hashes and integrity results remain local under:

`C:\Development\Rust\targets\eliot-search-standalone-study\evidence`

Raw external-repository source/search output is not copied into this repository. The next comparison
requires source-matched Windows x64 executables, an isolated short data root, verified exclusion of
secrets/generated/vendor files, and corpus-byte agreement with the recorded baseline. Development
tickets and accepted launch prerequisites remain necessary before product-package repairs.
