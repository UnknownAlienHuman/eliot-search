# Production acceptance checklist

This checklist is a go/no-go summary for the baseline ELIOT Search release. It does not replace
`swarm/gates.toml`, qualification registries, accepted receipts or `swarm/launch-state.toml`.

Any unchecked mandatory item is a release blocker.

## Authority and repository state

- [ ] `main` is the exact reviewed release source revision.
- [ ] All mandatory package handoffs are immutable, accepted and bound to the release revision.
- [ ] G0, G1, G2, G3, G4 and G5 receipts are accepted.
- [ ] `W7_LIFECYCLE` is accepted separately.
- [ ] No implementation ticket, writer lease, submission or review is left unresolved.
- [ ] Launch state names the exact active stage/wave and accepted artifacts.
- [ ] W10 remains disabled unless one independently accepted G6 candidate is intentionally shipped.
- [ ] Cargo, package, function, module, stage, coverage and package-map validators pass with no drift.

## Build and supply chain

- [ ] Exact stable Windows MSVC Rust toolchain is pinned.
- [ ] `Cargo.lock` is committed and matches the release build.
- [ ] Dependency source, license and advisory policy passes.
- [ ] No floating git, wildcard or automatic runtime dependency exists.
- [ ] Release build is produced from a clean checkout.
- [ ] Binary, Qdrant and other native artifact SHA-256 values are recorded.
- [ ] SBOM/license manifest is included.
- [ ] Optional dependencies are absent from the baseline or remain non-default and unauthorized.
- [ ] Release artifact version matches protocol/configuration/on-disk schema metadata.

## Installation and service lifecycle

- [ ] Clean installation creates the intended data root and restrictive ACLs.
- [ ] A second daemon cannot acquire the same live data root.
- [ ] Service start publishes readiness only after required dependencies are valid.
- [ ] Service stop drains requests and releases all leases, pins, handles and temporary state.
- [ ] Restart preserves accepted durable state and invalidates restart-scoped ephemeral state.
- [ ] Upgrade from every supported previous release succeeds.
- [ ] Interrupted upgrade follows a deterministic recovery or rollback path.
- [ ] Rollback restores a compatible accepted route/configuration/schema.
- [ ] Uninstall stops services, removes binaries and preserves or deletes product data only according to
      the selected retention policy.
- [ ] Installer performs no network download or silent runtime upgrade.

## Configuration and secrets

- [ ] All twenty configuration sections use the accepted owner validators.
- [ ] Defaults, file, environment and CLI precedence are deterministic.
- [ ] Unknown/duplicate sections and fields follow the accepted closed policy.
- [ ] Effective configuration is published only after all required action receipts succeed.
- [ ] Security, restart, rebuild, generation and optional-gate actions are not collapsed.
- [ ] Plaintext secrets are rejected.
- [ ] Secret references, values and leases do not appear in logs, errors, dumps or diagnostics.
- [ ] Configuration and secret rotation/reload behavior survives restart and fault injection.
- [ ] Optional capability configuration cannot self-authorize a provider.

## Source safety and authority

- [ ] Source admission is deny-by-default.
- [ ] Final opened handles are contained inside an admitted root.
- [ ] Reparse point, junction, symlink, rename and TOCTOU fixtures pass.
- [ ] Paths are never used as source identity.
- [ ] One source namespace has one active mutable identity/revision owner.
- [ ] Owner cutover is fenced and independently recoverable.
- [ ] Exact original or admitted revision bytes remain source truth.
- [ ] No reader, materializer, parser or document provider executes repository content.
- [ ] Unsupported, unreadable, unstable or oversized sources produce typed bounded outcomes.

## Durable state and recovery

- [ ] redb contains only control state, never searchable source content.
- [ ] Revision CAS residency/security/lifecycle domains are enforced.
- [ ] Every external mutation has durable intent, readback and recovery classification.
- [ ] Mutation timeout after a possible write remains `OUTCOME_UNKNOWN` until resolved.
- [ ] All supported control/CAS schema migrations pass from real previous data.
- [ ] Crash injection around intent, external write, readback and commit converges correctly.
- [ ] Quarantine prevents unsafe automatic recovery.
- [ ] Backups preserve ownership, residency and tombstone semantics.
- [ ] Restore cannot resurrect purged data.
- [ ] Rebuild can recreate projections from authoritative retained sources/revisions.

## Qdrant and publication

- [ ] Exact Qdrant server/client/profile artifacts are qualified for Windows.
- [ ] Process identity, ACL, containment, startup, readiness, shutdown and restart probes pass.
- [ ] Required Query API/schema/capabilities are verified at runtime.
- [ ] Each point belongs to exactly one projection membership.
- [ ] Point identity collision cannot overwrite another source/unit.
- [ ] Publication is globally serialized.
- [ ] Qdrant writes are acknowledged and read back before visible-epoch commit.
- [ ] Uncommitted generations/epochs are never observable as current.
- [ ] Epoch numbers are never reused within a generation.
- [ ] Pins protect active routes/epochs until completion.
- [ ] Ordinary reclaim deletes exact retired IDs only after all visibility/pin guards pass.
- [ ] Alias movement or worker readiness is never treated as commit.

## Query correctness and privacy

- [ ] Access/currentness filtering occurs before candidates, IDF, facets, counts and traces.
- [ ] Restrictive changes invalidate every affected rank leg.
- [ ] Stale/inaccessible candidates are removed before result projection.
- [ ] Every emitted candidate is backed by exact retained-revision readback.
- [ ] Results expose truthful ambiguity, partial, degraded, truncation and coverage state.
- [ ] Handle possession never grants access; expansion reauthorizes live state.
- [ ] Continuations are scope/request/plan/route/epoch bound and finite.
- [ ] Cancellation/disconnect releases requests, guards, pins, handles and continuations.
- [ ] Hot queries perform no durable control-store write.
- [ ] Source bodies, secrets, raw tokens, foreign membership and unrestricted paths do not leak.

## Current workspace and code structure

- [ ] Watcher/USN events are treated only as hints.
- [ ] Overflow/reset/restart opens an observation gap.
- [ ] Current-workspace capability is denied across an unresolved gap.
- [ ] Complete authoritative inventory is required before gap closure.
- [ ] Unsaved snapshots remain process-memory only.
- [ ] Unsaved/saved shadowing is applied before base retrieval and IDF.
- [ ] Overlay failure cannot reveal stale shadowed base results.
- [ ] Save transition admits a new immutable revision.
- [ ] Exact Rust parser artifact/profile/license is qualified.
- [ ] Rust enrichment executes no Cargo, rustc, build script, macro, shell, LSP or network activity.
- [ ] Tolerant syntax and `cfg` results retain explicit assurance and applicability.

## Comparison and exact proof

- [ ] Subject resolution preserves material ambiguity.
- [ ] Same-name, fork, mirror, lineage and `cfg` fixtures pass.
- [ ] Comparison remains descriptive and exposes evidence roles and gaps.
- [ ] No hidden “correct implementation” verdict is emitted.
- [ ] Exact scans use an authoritative frozen denominator, never Qdrant top-k.
- [ ] Every denominator item receives exactly one outcome.
- [ ] Exact negative claims are emitted only after complete successful denominator closure.
- [ ] Drift, unreadable, cancelled and security-invalidated items prevent false completeness.
- [ ] Regex/structural engine artifacts and profiles are exact, bounded and no-execute.

## Lifecycle, purge and restore

- [ ] Restrictive access/purge fences become effective before any later emission.
- [ ] Active contaminated requests are cancelled or degraded truthfully.
- [ ] Retention roots and mark manifests are complete and versioned.
- [ ] Sweep is resumable and exact.
- [ ] Ordinary reclaim and security/legal purge use separate owners and receipts.
- [ ] Purge tombstones survive restart, backup and restore.
- [ ] Durable handles and continuations are invalidated by lifecycle scope.
- [ ] Restore enters quarantine until authority/currentness/purge checks pass.
- [ ] Secure erase is not claimed without platform-specific evidence.
- [ ] Lifecycle crash/replay matrix is fully accepted.

## Protocol and client edge

- [ ] Frame size/version limits are checked before allocation.
- [ ] Local pairing/authentication and binding replay defense pass.
- [ ] Capability descriptors are binding-filtered and cannot grant authority.
- [ ] One request has bounded progress and exactly one terminal outcome.
- [ ] Disconnect/cancel stress leaves no leaked resources.
- [ ] Standalone CLI uses the protocol and never opens stores directly.
- [ ] Machine-readable output is stable and versioned.
- [ ] Human diagnostics are actionable and redacted.
- [ ] Optional ELIOT and Research adapters remain leaf mappings with no reverse authority.

## Product Pulse and SLOs

- [ ] Exact Windows environment, corpus, candidates, baselines and policy were frozen before results.
- [ ] Paired randomized schedule and minimum sample counts were respected.
- [ ] Warm exact keyword/navigation p95 is at most 100 ms.
- [ ] Warm single-scope lexical p95 is at most 200 ms.
- [ ] Warm cross-repository comparison p95 is at most 700 ms.
- [ ] First useful progressive card is at most 300 ms.
- [ ] Quality and recall metrics satisfy the pre-registered policy.
- [ ] CPU, memory, disk I/O, queue depth and background duty remain within accepted limits.
- [ ] False complete-negative claim count is zero.
- [ ] Stale leakage count is zero.
- [ ] Access leakage count is zero.
- [ ] Secret/content leakage count is zero.
- [ ] Protocol resource leak count is zero.
- [ ] Recovery correctness is 100% across mandatory fault cells.
- [ ] Independent reviewer accepted the exact evidence bundle and G5 verdict.

## Documentation and support

- [ ] Quick start covers DIRECT and qualified indexed modes.
- [ ] Configuration reference matches all twenty section schemas.
- [ ] CLI/protocol compatibility is documented.
- [ ] Source admission, privacy and non-goals are documented.
- [ ] Backup, restore, rebuild, purge and disaster recovery procedures are tested.
- [ ] Troubleshooting covers ownership, Qdrant, parser, currentness, protocol and capability failures.
- [ ] Artifact verification and version/support policy are published.
- [ ] Known limitations are explicit and are not relabeled success.

## Final decision

Release is allowed only when:

```text
every mandatory checkbox above is satisfied by immutable raw evidence
+ all required gate/receipt records are accepted
+ the release revision and artifact digests are exact
+ an independent reviewer records GO
```

A prose assertion, green unit test, successful build, static coverage report or optional worker readiness
does not constitute release acceptance.
