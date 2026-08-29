# Swarm readiness audit — 2026-08-28

## Verdict

The repository is ready to launch **P00/W0 only** after this scaffold merge. It is not ready to launch
all crates simultaneously and it is not an implemented product.

## Findings resolved

### 1. Broad family crates were unsuitable ownership units

The original family shape grouped C03-C07, C13-C17 and C18-C27 into three large packages with many
independent failure states. The current workspace preserves the architecture capability cells as real
leaf packages where dependency/test/replacement/context boundaries exist. Family directories contain
no Cargo package or forwarding implementation.

### 2. Package AGENTS files were directionally correct but not implementation-complete

The previous briefs named mission, dependencies and a few functions, but many agents would still have
needed the architecture master to discover exact primitives, state semantics, failure reasons and
required tests. Every package now has one bounded assignment under `swarm/assignments/` containing:

- owned and forbidden state;
- logical primitives and operations;
- invariant/precondition/postcondition expectations;
- typed degradation/failure surface;
- suggested internal module layout;
- deterministic/fault/security test seams;
- dependency handoff and size/split rules.

### 3. Cargo membership was being mistaken for launch authorization

The new `swarm/launch-state.toml` is the only current authority. W0 starts with `search-contracts`;
`search-domain` is conditional on its accepted contract handoff. Every later package is blocked even
though its empty scaffold exists in Cargo.

### 4. Daemon fan-in would have recreated a monolithic agent context

`eliot-searchd` ultimately composes most capabilities, but its implementation is now defined as
progressive feature layers. A W1 daemon writer receives only the owner/journal/protocol shell. Future
layers are enabled from accepted package handoffs rather than by reading all final dependencies.

### 5. Integration ownership was implicit

`swarm/INTEGRATION_OWNER.md` now owns root workspace/dependency/toolchain/lockfile/CI/generated-schema
and launch-state changes. Package writers cannot patch root files or cross-package contracts.

### 6. Optional depth was documented but not a machine gate

Model and document packages are explicitly blocked in launch state and in their assignments until an
accepted P15 Product Pulse, dedicated ADR and exact artifact/provider qualification.

### 7. Line control needed an earlier split trigger

The hard limit remains 10,000 hand-written Rust lines including local tests. Assignments target at most
7,500 `src/` lines and require design/split review before 8,500 total lines. A split still requires a
real boundary; forwarding-only crates remain forbidden.

## Deliberately unresolved for P00 integration

These are not filled with placeholders because they require executed qualification:

- exact stable Rust toolchain pin for the selected Windows/dependency set;
- `Cargo.lock`;
- exact external crate versions and source/license proof;
- exact Qdrant server/client patch and artifact SHA-256;
- CI activation and generated contract schemas;
- runtime, fault, security, performance and Product Pulse evidence.

P00 must produce real evidence for the first three. Qdrant qualification belongs to P05.

## Launch recommendation

1. Assign `search-contracts`.
2. Accept its public API/schema digest and contract fixtures.
3. Assign `search-domain` against that immutable handoff.
4. Let the integration owner pin toolchain/dependencies, generate the lockfile and run P00 workspace
   policy checks.
5. Publish the W0 receipt before changing `active_wave`.
