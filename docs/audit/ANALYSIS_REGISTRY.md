# Analysis registry

Current next step and evidence only. Algorithms stay with their owners.

<a id="resume"></a>
<a id="resume-after-interruption"></a>
## Resume

**R063 — local backend cutover and concrete agent routes, 2026-10-29.**
Inspected main: `7ed46f79f07a23f01f75f5e85a96248dc42e280b` (R062). Documentation only; no code/dependency removal is implied.

Next implementation: [Foundation](../implementation/README.md#1-foundation--invocation-settings-and-root), starting
at the [actual dispatch](../implementation/CODE_MAP.md#existing-dispatch-to-replace). Parse invocation once, route
plain inspect/init/open to the real root owner, then report factual status without earlier legacy interception.
The guide defines stop conditions and minimal check/Clippy; do not start every linked issue or require Qdrant here.

<a id="established-decisions--do-not-rediscover"></a>
<a id="decisions-and-owners"></a>
## Selected decisions

[Local scope](../architecture/LOCAL_SCOPE.md) is unchanged: one OS account, plain storage, child stdio and local
Qdrant. Public unused key wrappers may be removed with callers; private work and user bytes/keys remain intact.
[Bridge](../../crates/search-index-qdrant/search-qdrant-bridge/FUNCTIONS.md#connection-versus-index-readiness)
distinguishes connection, not-yet-indexed, damaged-current and verified-empty views. No readiness service is added.

<a id="coverage-ledger"></a>
<a id="changed-documentation"></a>
## Documentation changes

Removed the remaining public-key-layer preservation instruction from CLI/client. Consolidated Qdrant entrypoints:
local connection/schema stays in the bridge, publication in its owner, old managed-launch plans in history.
The first path needs no supervisor, key lease, mode selector or duplicate readiness probe. No remote-user scope.
Added actual short-circuit dispatch and Foundation stop conditions; made one-shot versus later live-session order
explicit. Current task issues must point to main's owners, not earlier refactor-branch copies or duplicated tables.

<a id="delivery-remaining"></a>
## Remaining implementation

Foundation → one retained file through real Qdrant and the actual CLI → later stages in the delivery guide.
Old crypto/pairing/supervisor code and dependency edges still exist until real consumers are cut over. #610 source
work remains separate. Preserve unpublished #598 work; it is not a selected integration. No product stage is closed.

<a id="evidence-status"></a>
<a id="execution-evidence"></a>
## Evidence

| Check | Result / limit |
|---|---|
| Exact inputs | All 16 edited files reconstructed and matched their GitHub blob IDs at the inspected main revision. Earlier archive used only for matching/reference, not as a current checkout. |
| Source read | Both current entry.rs bodies and root dependency pins read; dispatch order is statically observed, not executed. No fresh whole-repository caller census. |
| Documentation | Changed-page relative links/fragments, preserved old heading anchors and diff whitespace checked locally. Supporting targets inspected separately; no full fresh-main/legacy or external-link crawl. |
| Publication | Parent/candidate comparison must contain Markdown only; verify main and merged tree after landing. |
| Runtime | No Rust, Cargo/lockfile, config, workflow or test-source changes. No build, Clippy, Qdrant, Windows or product tests run. Existing donor selections were not re-audited. |

<a id="evidence-index"></a>
<a id="prior-evidence"></a>
## Previous evidence

[R062 registry](https://github.com/UnknownAlienHuman/eliot-search/blob/7ed46f79f07a23f01f75f5e85a96248dc42e280b/docs/audit/ANALYSIS_REGISTRY.md) retains earlier decisions/checks.
Update the next connected change, exact revision and actual checks; do not append another audit diary or spec.
