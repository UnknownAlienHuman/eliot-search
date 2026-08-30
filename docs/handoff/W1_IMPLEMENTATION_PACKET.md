# W1 process and control implementation packet

**Stage / wave:** P01–P02 / W1  
**Status:** `BLOCKED` until the accepted W0 receipt and exact package tickets.  
**Goal:** construct the smallest DIRECT-capable process/control shell without source preparation,
indexing or query business behavior.

## Package order

```text
accepted search-contracts/search-domain/search-ports
        ↓
search-config
        ↓
search-runtime-owner   search-os-secrets   search-control-redb
        ↓                     ↓                   ↓
search-provider-protocol
        ↓
eliot-searchd wave1 shell + eliot-search thin client
```

Independent packages may run in parallel only after exact accepted direct handoff/API digests. Daemon
composition starts after every directly used public port/configuration digest is immutable.

## One-agent package packets

| Package | Primary packet | Write scope |
|---|---|---|
| `search-config` | `crates/search-config/FUNCTIONS.md` | `crates/search-config/**` |
| `search-runtime-owner` | `crates/search-runtime/search-runtime-owner/FUNCTIONS.md` | package only |
| `search-os-secrets` | `crates/search-runtime/search-os-secrets/FUNCTIONS.md` | package only |
| `search-control-redb` | `crates/search-control-redb/FUNCTIONS.md` | package only |
| `search-provider-protocol` | `crates/search-provider-protocol/FUNCTIONS.md` | package only |
| `eliot-searchd` | `bins/eliot-searchd/FUNCTIONS.md` | package only |
| `eliot-search` | `bins/eliot-search/FUNCTIONS.md` | package only |

Exact paths are machine-registered in `swarm/function-packets.toml`. Presence there is not launch
authorization.

## W1 lifecycle

```text
UNINITIALIZED
→ ROOT_OWNER_ACQUIRED
→ SECRETS_READY
→ CONTROL_OPEN_AND_VERIFIED
→ CONTROL_SNAPSHOT_PUBLISHED
→ PROVIDER_ENDPOINT_READY
→ DIRECT_SHELL_READY
→ DRAINING
→ STOPPED
```

Any unprovable root/process/control identity enters explicit degraded/quarantined state. A responding
PID, pipe, port or existing directory is never ownership proof.

## Ownership boundaries

- `search-config` owns pure parsing/layering/provenance/redaction/fingerprint/diff/composite planning;
  the daemon captures I/O and capability owners validate semantic sections.
- `search-runtime-owner` owns one local data-root owner epoch/guard and abandoned-owner classification;
  it does not drain daemon services itself.
- `search-os-secrets` owns opaque bound references, guarded use, leases, rotation and logical deletion;
  it does not launch processes or authenticate sessions.
- `search-control-redb` owns bounded technical durable state and immutable control snapshots; it is never
  a source/search/query database.
- `search-provider-protocol` owns bounded local framing/session/binding/request lifecycle; it owns no
  source/index/query authority.
- `eliot-searchd` constructs private adapters and orders lifecycle; it does not reimplement package logic.
- `eliot-search` is a protocol client only and has no direct store path.

## Core invariants

- one installation incarnation and one active owner per local data root;
- managed and standalone modes never co-own a root;
- owner/process/executable/root identity, not PID/lock alone, fences reuse;
- plaintext secrets are impossible in public types, config, argv, environment snapshots, logs, metrics,
  panic/error text and receipts;
- purpose/user/installation/incarnation/generation binding is enforced for every secret use;
- control durability and verified-or-quarantine migration floors cannot be disabled;
- hot read admission creates zero durable control writes/idempotency rows;
- committed control state precedes in-memory snapshot/readiness publication;
- protocol frame/in-flight ceilings remain finite and unauthenticated second endpoints do not exist;
- configuration is authoritative only after every required action receipt succeeds;
- no W2+ capability is initialized or advertised by the wave1 shell;
- shutdown stops admission and releases requests/dependencies before owner release.

## Required W1 tests and evidence

- concurrent second owner denial, stale/ambiguous record, PID reuse, executable replacement and owner-
  epoch crash/reopen matrix;
- secret create/use/lease/rotate/delete crash matrix and exhaustive side-channel canaries;
- control create/open/migration/transaction/unknown-outcome/power-loss/quarantine fixtures;
- 10,000 hot reads with zero redb writes;
- deterministic configuration parse/layer/fingerprint/diff/redaction/composite-action fixtures;
- provider framing/hello/pairing/replay/in-flight/backpressure/cancel/disconnect fixtures;
- daemon crash at every startup/control snapshot/endpoint/shutdown boundary;
- wave1 feature/dependency graph proves no W2+ construction;
- CLI/worker/adapter direct-store and public vendor-type guards.

## Hard stops

- W0 API/handoff receipt absent or moving;
- plaintext secret or secret-bearing diagnostic/process-launch path;
- second owner, blind orphan attach or remote/network data root;
- redb corpus/search/query content or hot-read durable mutation;
- unverified migration/corruption repaired in place instead of quarantine;
- protocol resource/authentication floor weakened;
- mixed partially applied configuration published;
- daemon public API exposes concrete redb/secret/platform types;
- W2+ capability reported available;
- package writer edits outside exact package scope or selects a shared dependency/artifact.

## Handoff

Each package publishes exact commit, public API/configuration digest, state-owner inventory, operation/
error inventory, deterministic/property/fault results, unavailable evidence, dependency set and line
count. Integration publishes one W1 receipt only after composed shell evidence is independently reviewed.
Compilation and structural validation are not a wave receipt.
