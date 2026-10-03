# Control-record byte verifier review

Scope: integration tooling only. This report is diagnostic evidence, not an issued control record,
package review receipt, qualification, handoff, gate or wave acceptance.

Final reviewed source: `6e57c317233e138e178db634696cbfdc8f6bf124`, comprising the initial six-file
verifier commit `b2876eb348da898d090cc9da5741c1da5e6a94eb` and its two-file boundary correction.
The initial implementation originated at writer commit `24ab0876ecff1bbf990a5ed140ee72403e865187`.
Independent reviewer: `/root/contract_auditor`, separate from the writer and root corrective author.
Verdict: **no remaining code blocker for this bounded byte-verification scope**.

## Finding and correction

The initial verifier accepted a file beginning immediately with `[signature]` when its embedded digest
was SHA-256 of the empty string. That violated the required byte range ending with the LF immediately
before the signature header. Both the independent reviewer's external harness and the root regression
reproduced the actual acceptance. The correction rejects offset zero and requires the preceding LF
before hashing. Short-circuit evaluation prevents indexing below byte zero.

The verifier computes two distinct digest types over original bytes. It rejects noncanonical byte
encodings, comments, invalid TOML, missing/ambiguous signature headers, later table headers and embedded
payload-digest mismatch. A positive caller byte ceiling is checked before parsing. Its two existing
advisory consumers already read through the planner's 4 MiB source ceiling, and their compatibility
wrapper applies that same ceiling. This is not a global budget for future mutation records.

## Executed checks

Rust/Cargo 1.98.0, Windows x64 MSVC. Commands ran locally with the isolated target directory
`C:\Development\Rust\targets\eliot-search-record-bytes`.

| Check | Observed result | Raw evidence |
| --- | --- | --- |
| New verifier tests before implementation | Exit 101: module absent | `evidence/control-record-bytes-red.*` |
| Initial verifier target | Exit 0: 8 passed | `evidence/control-record-bytes-final.*` |
| Focused historical marker-vector rejection | Exit 0: 1 passed | `evidence/ticket-parity-signed-payload.*` |
| New no-preceding-LF regression before correction | Exit 101: actual invalid acceptance | `evidence/control-record-bytes-boundary-red.*` |
| Verifier target after correction | Exit 0: 9 passed | `evidence/control-record-bytes-boundary-green.*` |
| Explicit two-file rustfmt and diff check | Exit 0 | Executed; separate raw transcript not retained |

Historical captured fixture bytes were preserved. The focused historical test now correctly treats
marker-only, invalid-TOML captures as negative inputs to the stricter verifier. The full workspace and
unchanged product builds were not repeated. Raw stdout, stderr and exit files were copied without text
conversion and matched their local originals by SHA-256.

## Proof and limits

Before the corrective edit, a scoped Codebase Memory query identified the prior helper paths. Its cached
bodies predated the new verifier, so live Git/source anchors were used for the offset, slice and caller
ceiling proof. The authoritative source was the exact integrated commit, not stale graph content.
Native ELIOT diagnostic tools were unavailable; no ELIOT verification or memory writeback is claimed.

The independent reviewer inspected string escapes, multiline strings, arrays and inline tables. Existing
tests exercise multiline fake headers and nested signature references. Dedicated escaped-quote and
multiline-array fixtures are optional future coverage; no defect in those cases was observed.

This helper verifies a byte envelope and digests only. It does not verify complete schemas or field
order, signature authenticity, trusted actors, qualified profiles, store readback, issued assignments,
leases, acknowledgements or implementation authority. No product package or control record was written.
