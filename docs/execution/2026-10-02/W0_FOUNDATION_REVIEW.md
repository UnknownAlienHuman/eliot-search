# W0 foundation independent review

**Verdict: NOT_ACCEPTED**

**Audited source commit:** `9d61b759189464a01b93ca4efcb80c5398344bb4`

**Initial review date:** 2026-10-02

**Follow-up update:** 2026-10-03

**Scope:** Read-only review of the `search-contracts` P00 contract and its package source/tests. This file is the only audit artifact created by this review. No source, registry, Cargo manifest, ticket, lease, or handoff record was changed.

## Findings

### 1. Typed wire round-trips and closed decoding are absent

The package assignment requires “all records round-trip” and “invalid-tag/unknown-field rejection” (`swarm/assignments/search-contracts.md:34-37`). The crate currently exposes canonical JSON/CBOR operations over generic `CanonicalValue` (`crates/search-contracts/src/canonical.rs:256-310`) and a frame helper over raw UTF-8 payload bytes (`crates/search-contracts/src/protocol.rs:283-328`). `ProviderEnvelope` has in-memory validation (`protocol.rs:231-280`), but no typed envelope/body decoder or encoder was found in the package. Existing round-trip fixtures cover generic values and frames, not each public record (`crates/search-contracts/tests/conformance.rs:358-454,535-544`).

This leaves the declared provider-wire and record contracts without executable typed wire conversion or record-level unknown-field checks. The conformance suite does cover enum wire registries, shape validation, generic `CanonicalValue` round-trips, and frame validation (`crates/search-contracts/tests/conformance.rs:358-454,535-544,558-580,1137-1457`). Its `ProviderEnvelope` tag/version test exercises the in-memory validator; the package has no typed JSON/CBOR record conversion or decoding path. Close the gap with typed conversions for the owned public schemas and per-record round-trip, invalid-tag, and unknown-field fixtures, or have the integration owner explicitly reassign that behavior before freezing the API.

### 2. Exported authorization schemas are outside the P00 contract and module registry

`crates/search-contracts/src/lib.rs:42,56` publicly declares and re-exports `authority`. That module exposes `ProviderBindingRecord`, `AuthoritativeGrantPolicy`, and `StandalonePolicyRecord` (`crates/search-contracts/src/authority.rs:36,89,175`). I found no definitions for these records in the manifest-declared P00 files. The W0 module registry enumerates 13 modules and omits `authority` (`swarm/modules/w0.toml:9-17`). These authorization shapes need closed field/visibility/bound/owner contracts and a registered module before they can enter the accepted API digest; otherwise they must be removed or re-homed by the integration owner. This finding overlaps existing open issue #194, which concerns durable binding/grant-policy authority; no GitHub issue or comment was created by this review.

### 3. P00 manifest link omission — corrected in the current integration diff

At the audited source commit, `docs/contracts/p00/PROTOCOL_AND_LIFECYCLE.md:271-275` refers readers to `PUBLICATION_GUARDS_CORRECTION.md`, which was absent from `docs/contracts/p00/manifest.toml:6-20`. Integration commit `8dc3ef19bbd128bcc2a23a2da870b7c76367bfcd` closes this omission consistently: it adds the correction to the manifest and P00 README and adds the same exact source to `swarm/context-drafts/p00/search-contracts.toml`, updating the declared source count from 20 to 21. The corrected count remains below the P00 ceiling of 24. The draft still says `UNMATERIALIZED_DRAFT`, `claimable = false`, and `authorizes_implementation = false`; this edit does not issue a ticket or lease. This finding is corrected in the integration commit and is not a remaining source-pack blocker.

## Integration-diff checks

- `Cargo.lock` changes by one added `tokio` dependency edge under `eliot-searchd`; no version, source, or checksum entry changed (`git diff --numstat -- Cargo.lock`: `1  0`). The daemon manifest already declares exact `tokio = 1.53.1` under `[dev-dependencies]` (`bins/eliot-searchd/Cargo.toml:174-177`).
- `cargo metadata --locked --offline --format-version 1` exited 0.
- The P00 correction-doc closure described in finding 3 was independently checked across the manifest, README, and context draft in integration commit `8dc3ef19bbd128bcc2a23a2da870b7c76367bfcd`.
- **Preflight interpretation/decision point:** the draft includes the exact package-only selector `swarm/launch-state.toml::authorized_packages[search-contracts]` (`swarm/context-drafts/p00/search-contracts.toml:54-60`). The integration owner identifies this as classification metadata needed for mandatory launch/prerequisite checks, not a mount of the whole launch-state file. The generic writer-read list does not enumerate launch-state (`AGENTS.md:10-23`, `swarm/ASSIGNMENT_PROTOCOL.md:32-43`), while the authority map assigns launch/readiness decisions to that registry (`AGENTS.md:55-57`). This review did not establish a scope violation; preserve the exact package-only selector and verify the materialized context does not widen it.

## Targeted test result

Command run once from the audited workspace, with build output directed outside the workspace:

```powershell
$env:CARGO_TARGET_DIR = 'C:\Development\Rust\targets\eliot-search-luna-contracts'
cargo +1.98.0 test --locked -p search-contracts --lib
```

Raw output:

```text
   Compiling search-contracts v0.0.0 (C:\Development\Rust\projects\eliot-search\crates\search-contracts)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 1.54s
     Running unittests src\lib.rs (C:\Development\Rust\targets\eliot-search-luna-contracts\debug\deps\search_contracts-6c073eb1579efe81.exe)

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

The command exited 0 but discovered zero library unit tests. It did not run `tests/conformance.rs` or validate the missing typed round-trip/closed-decoding behavior. This result does not qualify the package.

### Conformance integration suite

Command run once from the audited workspace with the same external build directory:

```powershell
$env:CARGO_TARGET_DIR = 'C:\Development\Rust\targets\eliot-search-luna-contracts'
cargo +1.98.0 test --locked -p search-contracts --test conformance
```

Raw output was saved outside the repository at `C:\Development\Rust\targets\eliot-search-luna-contracts\W0_FOUNDATION_CONFORMANCE.raw.txt`.

Exact result and exit:

```text
test result: ok. 50 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
EXIT_CODE=0
```

This suite passed. It does not supply typed record JSON/CBOR conversion or round-trip behavior, so finding 1 remains open. The existing tests validate generic canonical values, enum registries, and in-memory record rules; unknown-field rejection is exercised on generic `ClosedCanonicalObject`, not by decoding each public schema. Integration commit `8dc3ef19bbd128bcc2a23a2da870b7c76367bfcd` changed no `crates/search-contracts` source or test files, so the tested package tree matches the audited source commit.

## Acceptance boundary

The manifest-link correction and lockfile edge do not resolve findings 1 or 2. The package remains `NOT_ACCEPTED`; no package/API handoff, gate receipt, or wave receipt is implied. The launch/ticket state is unaffected by this review.
