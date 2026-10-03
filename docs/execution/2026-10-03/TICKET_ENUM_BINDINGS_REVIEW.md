# Adopted ticket enum bindings — corrected implementation

**Accepted integration source:** `82315138f337ecbcab5ccb1a3f01b67330930572`.
Independent Luna Max source review accepted exact candidate
`b585f835a941a12e1db6afa2dc1d14ccdfb3ec54`. One combined Cargo command passed **18/18** tests,
and one structural PowerShell command exited 0 with no errors. These results establish bounded
integration behavior, not complete record validation, qualification, issued authority or a package handoff.

The implementation follows only the [accepted four vocabularies and nine bindings](../../handoff/changes/2026-10-03-ticket-enum-adoption.md).
It preserves all canonical field arrays, unrelated rules, type-registry format 2, control-registry
version 4 and record schema 1. The registered type count is now 51. Other proposed command links and
exactly-once coverage rules remain unadopted. The qualified-profile parser is a separate syntax-only
implementation; its planned type-registry addition is not included in this count.

The Rust checker reads `[current_disposition].registered_types` correctly and validates bindings as
owner/field/target tuples. Four legitimate evidence-class owners and two acceptance-class owners may
share the same rule string; wrong owners and duplicate bindings still reject. Its pure value function
checks the nine exact owner/field pairs against canonical case-sensitive allowlists, with a 128-byte
combined token bound. Registry input is bounded to 80 KiB. Its scope remains `NON_AUTHORITATIVE`.
The PowerShell checker now checks the complete four allowlist literals, canonical boolean syntax and
accepted ownership. It remains a structural checker and does not claim to parse all TOML or validate
complete record instances.

## Executed checks

At the clean candidate worktree, the command was:

```text
cargo +1.98.0 test --locked --offline --no-fail-fast -p xtask --test control_record_enum_bindings --test control_record_instance_profiles --test ticket_issuance_builder
```

| Target | Result |
| --- | --- |
| Adopted enum definitions, bindings and pure values | 7/7 |
| First-four instance profiles | 8/8 |
| Actual-repository issuance builder | 3/3 |

The [Cargo result](evidence/ticket-enum-bindings/b585f835a941a12e1db6afa2dc1d14ccdfb3ec54/result.json)
records source, exact cwd/command/environment, clean states, UTC time, exit and original stream hashes.
The [raw stdout](evidence/ticket-enum-bindings/b585f835a941a12e1db6afa2dc1d14ccdfb3ec54/cargo-test-stdout.txt)
contains all three target results; stderr retains existing warnings.

The structural command was `pwsh -NoProfile -File tools/validate-ticket-issuance-contracts.ps1 -Json`.
Its [result](evidence/ticket-enum-bindings/b585f835a941a12e1db6afa2dc1d14ccdfb3ec54/ps-structural-result.json)
records the exact cwd and default script-root resolution. It reports 51 types, 8 records, 211 fields,
9 signature references, 28 workflows, **0 issued records**, and `errors=[]`. It does not execute the
issuance qualification probes.

Integration compared eight changed Git blobs exactly against the reviewed candidate. `lib.rs` differs
only by retaining the separately accepted `qualified_profile_id` module declaration; removing that
single line yields the candidate's exact line sequence. Both new parser/test files remain present.
No passing target, CLI or whole-workspace suite was repeated after integration.

## Preserved history and remaining work

The [restart record](RESTART_RECOVERY.md) preserves the original CLI compile failure and cb468's
0/5/20-error outcomes, including skipped targets and source-provenance limits. The correction repairs
their causes without relabeling them. The [copy manifest](evidence/ticket-enum-bindings/b585f835a941a12e1db6afa2dc1d14ccdfb3ec54/copy-manifest.json)
preserves 24 final capture files byte-for-byte; Git text conversion is disabled in the evidence folder.

The full canonical instance validator, profile/store/actor trust, ACK encoding, authoritative mutation
runtime and independently executed qualification remain unresolved. No ticket, lease, handoff, gate,
wave, launch classification, product source or qualification verdict changed. Actual standalone Search
retrieval and W3 bridge qualification remain unavailable.
