# Local Git artifact adapter: first implementation boundary

**Implementation deferred:** the project owner's 2026-10-03 clarification places this agent-management
component after Eliot Memory OS startup. Current implementation priority is the Qdrant wrapper and
standalone Search. Preserve the component's draft work for that later phase; it does not gate current
package changes.

**Decision:** accepted for bounded integration-tooling implementation only. This freezes the small
object preparation/readback component below. It does not adopt the complete local issuance profile,
qualify a store instance, authorize control-record publication, or create an assignment or lease.

The accepted executable-orchestration ADR permits Git commits and append-only receipts. The exact
artifact-store design blob `sha1:5279eb3087ed9dac66f448f0fec65eaa274417e0`, at proposal source
`af568db0893f080d15cf6e5bc503040ee7205c3b`, supplies the candidate's locator, exact-byte and create-only
object semantics. Only this component's definition is accepted here; the profile draft remains a
proposal with unavailable qualification. Missing keys and future qualification outcomes do not
prevent implementation or isolated tests of this component.

## Owned scope and inputs

Implementation ownership is integration-owner/native Rust tooling, in
`xtask/src/local_git_artifact_store.rs`, its private directory if needed,
`xtask/tests/local_git_artifact_store.rs`, and one additive module declaration in `xtask/src/lib.rs`.
No CLI mutation command is introduced. No Cargo dependency or product-package source changes.

The component takes an explicitly configured absolute Git executable, expected executable SHA-256
and exact version string, explicit repository root and expected Git object format. It rejects missing
or mismatching identities. There is no download, executable selection, provider fallback, namespace
inference, actor registration or trust provisioning. The tested executable observation on 2026-10-03
is `C:\Program Files\Git\cmd\git.exe`, `git version 2.55.0.windows.3`, executable SHA-256
`7b7971dd13f0c3a284e538601f2f9770b3a87dfaccb5fb52d68141c67ed22364`; this observation is not
qualification of Git's complete loaded runtime or a configured store instance.

## Operations

1. Prepare one ordinary blob from bounded exact binary input. Compute independent SHA-256 and
   `sha256-<64 lowercase hex>` artifact ID. Write with `hash-object -w --stdin --no-filters`, check the
   full tagged blob ID and read back the object's type, length and exact bytes. Existing identical
   objects may be reused; a mismatch is an integrity failure. No tree, commit, ref, index or working
   file is changed. Object preparation does not publish an artifact or authorize its use.
2. Verify one prepared blob by its complete descriptor: full algorithm-tagged blob ID, SHA-256,
   artifact ID and byte count. Reject foreign formats, abbreviations, malformed descriptors,
   non-blob objects, size/digest mismatch and missing objects. Read raw object bytes, never checkout
   files, filters, replace objects, revision expressions or a moving ref.
3. Resolve and verify a committed artifact only from an exact full tagged commit and its derived path
   `swarm/control-artifacts/v1/sha256/<first-two-hex>/<64-lowercase-hex>.bin`. Require an ordinary
   non-executable blob at exactly that path and the descriptor's exact blob ID, length and SHA-256.
   Reject aliases, symlink modes, directories, foreign formats, moving names and missing entries.

The API reports these object-level outcomes explicitly as non-authoritative. It does not interpret
record schemas or signatures, validate actors or leases, declare a context materialized, create a
durable operation receipt, advance a ref, or return a control-operation success. Atomic append-only
commit publication, ref compare-and-swap, full-profile lifetime budgets and recovery are a separate
implementation step after their complete input/receipt contracts are frozen.

## Finite bounds and failure behavior

This first component has an explicit 4 MiB input/blob limit, within the design's 100 MiB context
ceiling. Larger artifacts return a typed unsupported-size result before object preparation; callers
must not label that result successful materialization. It has a 16 MiB operation-memory ceiling,
bounded Git output and diagnostics, and at most 120 seconds per operation. Subprocesses use argument
arrays, raw pipes, a sanitized Git environment and explicit absolute runtime identity. Inherited
repository redirection, config injection, filters and replacement-object settings must not redirect
or reinterpret the named repository/object. No shell is used and raw artifact bytes are not disclosed
in errors.

Cancellation/deadline before a possible object write returns a typed stopped result. If preparation
may have written before cancellation, timeout or lost response, preserve an object-write-unknown
outcome until exact readback resolves it. Never claim the object absent, published or safe to retry
from a killed child alone. Verification operations cannot create an object. OS handle/ACL containment,
loaded-runtime qualification and concurrent hostile repository replacement remain prerequisites for
authoritative profile use; this component's path checks do not establish them.

Tests use freshly created isolated local Git repositories only. Cover binary and CRLF preservation,
same-byte reuse, exact descriptor/commit/path readback, wrong size/digest/OID/object kind, missing
objects, malformed or moving identities, symlink entries, configured runtime mismatch, input/output
bounds, inherited Git redirection and deadline/cancellation classification. Keep original command,
source commit, clean state, runtime, timestamps, stdout/stderr and exit evidence. One focused target
is sufficient after source review; unavailable fault cases remain explicit. No test changes the main
repository's objects, refs or control state.

Git's [hash-object documentation](https://git-scm.com/docs/git-hash-object) defines the raw-byte
`--no-filters`/stdin behavior; [cat-file](https://git-scm.com/docs/git-cat-file) documents direct object
type/size/content inspection. Official documentation was checked on 2026-10-03. These references
support the plumbing definition, not an executed profile qualification.
