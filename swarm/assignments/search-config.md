# `search-config` implementation packet

**Path:** `crates/search-config`  
**Capability:** shared configuration mechanics  
**Delivery:** W1 / P01  
**Gate:** BLOCKED until accepted W0 contracts handoff  
**Direct public handoffs:** `search-contracts`

Apply `../ASSIGNMENT_PROTOCOL.md`. Read only this assignment, package/root instructions,
[`../../crates/search-config/FUNCTIONS.md`](../../crates/search-config/FUNCTIONS.md),
[`../../docs/config/CONFIGURATION_1.0.md`](../../docs/config/CONFIGURATION_1.0.md),
[`../../docs/config/RECONFIGURATION_1.1.md`](../../docs/config/RECONFIGURATION_1.1.md),
[`../../config/sections.toml`](../../config/sections.toml) and accepted dependency handoffs.

## Mission

Provide deterministic, I/O-free parsing/layering, provenance, redaction, fingerprinting, diff and
composite reconfiguration planning. Capability packages retain ownership of typed settings and runtime
application.

## Owns

- bounded UTF-8 TOML document parsing after caller-provided byte acquisition;
- `ConfigSource`, layers, provenance and explicit reset markers;
- deterministic `defaults < file < environment < CLI` precedence under field allowlists;
- section descriptor registry and duplicate/unknown rejection;
- canonical effective snapshot and `ConfigFingerprint`;
- stable `ConfigDelta` and composite `ReconfigurationPlan`;
- redacted effective-config diagnostics.

## Must not own

- file, environment, CLI or clock acquisition;
- capability-specific settings structs/default policy/runtime state;
- secret plaintext, secret resolution or credential transport;
- optional-provider authorization;
- applying lifecycle, security, rebuild, generation or gate actions;
- reducing multiple obligations to one lossy severity enum.

## Required operations

The exact behavior is in `FUNCTIONS.md`:

1. `parse_document`
2. `register_sections`
3. `merge_layers`
4. `project_section`
5. `assemble_effective`
6. `fingerprint`
7. `diff`
8. `plan_reconfiguration`
9. `redacted_view`
10. `validate_environment_key`

## Composite action semantics

`ReconfigurationPlan.required_actions` is a set, not a scalar maximum. It may simultaneously contain:

```text
APPLY_LIVE
SECURITY_BARRIER
RESTART_DEPENDENCY
DRAIN_AND_RESTART
NEW_COLLECTION_GENERATION
REBUILD_PROJECTION
GATE_REQUIRED
REJECT
```

`NOOP` is the empty set. `REJECT` blocks all execution. `GATE_REQUIRED` blocks activation until named
receipts exist. Security, restart, rebuild and generation obligations may coexist and none may be erased
by another action. The daemon topologically orders prerequisites and publishes the candidate fingerprint
only after every required receipt succeeds.

## Required invariants

- unknown section/key, duplicate key/table, unsupported version and wrong type fail closed;
- a higher-precedence source may override only an explicitly allowed field;
- fixed security floors cannot be weakened;
- plaintext secrets are invalid in every layer;
- one invalid section rejects the entire candidate snapshot;
- equal canonical inputs produce byte-identical fingerprints, deltas and plans;
- failed or partially executed reconfiguration never publishes a mixed snapshot;
- optional model/document settings remain blocked without gate, ADR, artifact and feature receipts;
- parser, merge, fingerprint, diff and plan operations perform no I/O.

## Typed failures

`CONFIG_PARSE_FAILED`, `CONFIG_SCHEMA_VERSION_UNSUPPORTED`, `CONFIG_DUPLICATE_KEY`,
`CONFIG_UNKNOWN_SECTION`, `CONFIG_UNKNOWN_KEY`, `CONFIG_OVERRIDE_NOT_ALLOWED`,
`CONFIG_SECURITY_FLOOR_VIOLATION`, `CONFIG_SECRET_PLAINTEXT_FORBIDDEN`, `CONFIG_SECTION_CONFLICT`,
`CONFIG_SECTION_INVALID`, `CONFIG_PROFILE_NOT_AUTHORIZED`, `CONFIG_RECONFIGURATION_REJECTED`.

## Exit evidence

- layer precedence and override allowlist;
- duplicate/unknown/wrong-type fail-closed fixtures;
- secret rejection and redacted-view non-disclosure;
- deterministic fingerprint/delta/composite-plan fixtures;
- simultaneous security+restart and generation+rebuild obligations preserved;
- failed step never publishes candidate fingerprint;
- optional profile cannot self-authorize;
- package-section collision rejected;
- I/O-free dependency and behavior proof.

## Suggested modules

```text
source.rs
document.rs
registry.rs
merge.rs
section.rs
snapshot.rs
fingerprint.rs
diff.rs
plan.rs
redact.rs
error.rs
```

Target `src/` ≤5,500 lines; split review before 8,500 total; hard stop at 10,000 including local tests.
