# Codex restart recovery — 2026-10-03

**Status: PARTIAL_PROGRESS.** The existing completion Goal is active. The first live check after the
user reported a restart found clean `main` at `bac90ff7753ef7d4d107836c60480df216acfe7f`. A successful
`git fetch origin` confirmed the same remote `main` commit. The live collaboration catalog contained
only the root agent; previous agent execution was not assumed to have survived. Fresh scoped agents
were explicitly requested with model `gpt-6-luna` and reasoning effort `max`.

The saved worktrees and their committed candidates survived. Recovery reads exact commits,
raw captures and process state; historical green results do not verify a newer candidate.

## Recovered failures

- Candidate `1de0eb4cbfa36b7c9d1cc2fa9e97bca4a0d0a49e` has a retained CLI compile failure,
  exit 101, caused by comparing `str` with `&str`. The CLI did not execute its validator.
- Candidate `cb468257cf77c4ef31a884138d4f7b387b277b88` contains the one-line compile fix,
  but its retained combined Cargo invocation exited 101: the new enum target ran **0 passed / 5 failed**.
  Cargo stopped before the instance-profile and ticket-builder targets. The older 17/17 result at
  `166a4a2f59cc21998293f50071efa774220c4501` does not cover this candidate.
- The cb468 structural PowerShell invocation exited 1 and reported 20 errors, 51 types, 8 records,
  211 fields, 9 signature refs, 28 workflows and zero issued records. Its command capture omits cwd.

The [pre-restart capture manifest](evidence/ticket-enum-bindings/pre-restart-capture-manifest.json)
preserves all 17 original capture files byte-for-byte. Source association is the worker's retained
`source-sha.txt`; contemporaneous cwd/clean Git status were not captured. Recovery did not invent
missing provenance, run the skipped targets, or substitute an older pass.

Root inspection and independent source review identified the causes before another run: the Rust
checker reads `registered_types` at the root instead of `[current_disposition]`; both checkers wrongly
require a rule string to occur only once globally even when accepted owner/field pairs share it; and
the new PowerShell allowlist check extracts quoted values without checking comma syntax. Corrections
are assigned to one writer in the existing isolated worktree, with one bounded final Cargo invocation
and one structural invocation after committing the corrected source.

## Additional verified progress

The [qualified-profile syntax decision](../../handoff/changes/2026-10-03-qualified-profile-id-adoption.md)
accepts only the independently reviewed identifier grammar. A separate integration writer implements
its bounded pure parser. Profile registration, trust and qualification remain separate.

The [CNG diagnostic](CNG_PRIMITIVE_DIAGNOSTIC.md) and its immutable copied evidence were independently
reviewed, including committed Git-byte equivalence at `b728382a018235c2a33f42f94bb43c46e7c9e04c`.
No compiler or probe was repeated during recovery. Both diagnostic attempts remain visible.

The bootstrap proposal is being corrected to support a locally managed integration-owner trust pin
under an explicit trusted-host assumption and exact cross-artifact bindings. An external signing
service is not an existing normative requirement. No root/actor key, pin, credential or authority
has been created. ACK and P00 authority proposals remain unaccepted pending their scoped corrections
and independent review; they do not authorize package implementation.

The native Codebase Memory 0.9.0 CLI returned scoped graph results. Exact Git sources remain the
authority where the indexed snapshot is older. ELIOT/codebase-memory MCP tools are not exposed in the
current catalog; no Governor execution or memory writeback is claimed.

## Runtime boundary

The installed Qdrant 1.19.0 executable remains 84,184,576 bytes with SHA-256
`369C562EAE3D89333A13ABFDB522FA209E3F587C1217A1059D817E80814EA9D4`. The recovery process observation
found no Qdrant, Search daemon/CLI, Cargo, rustc or diagnostic process, and no listener on 16333–16335.
The previous basic CRUD/restart evidence remains valid for its narrow diagnostic scope; W3 is still
unqualified. Product build blockers and actual Search retrieval remain unresolved. The standalone
repository study retains its ordinary-search baseline; it is not a Search quality result.

No product package, launch state, qualification verdict, issued ticket or lease changed during this
recovery. Governor and Memory OS remain absent from the standalone runtime requirements. Verification
stays local, and no GitHub Actions workflow was dispatched.
