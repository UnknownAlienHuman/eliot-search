# ELIOT Search agent packages

This folder contains package source and templates, not active client configuration or installation proof.
The canonical [skill](skills/eliot-search/SKILL.md) is shared; the single Rust
[MCP consumer](../../bins/eliot-search-mcp/src/entry.rs) uses sibling eliot-searchd.exe through the existing SDK.

## Build artifacts after the matching Release build

Use an owner-verified, matching Windows Release set of MCP, daemon and operator CLI with retained source/build pins. The builder does not
compile binaries, substitute fixtures or prove that equal versions came from the same source. It reads the
three actual --version responses and requires them to match both package manifests. Version 0.0.0 currently
matches [workspace metadata](../../Cargo.toml); version changes must update the manifests explicitly.

Run with PowerShell 7 on Windows only after all three binaries are available. Replace every example path; the output's
parent must exist and OutputDirectory must be a fresh absolute, stable location outside the checkout, input
binaries and application dataset. Dataset/output/URL must not contain client macro syntax; unsupported literal
escaping is refused rather than guessed:

~~~powershell
$packageOptions = @{
    BinaryDirectory = 'C:\example\verified-release'
    DataRoot = 'C:\example\search-data'
    QdrantUrl = 'http://127.0.0.1:6334'
    OutputDirectory = 'C:\example\agent-packages\eliot-search-operator-root'
}
& .\integrations\eliot-search\scripts\build-packages.ps1 @packageOptions
~~~

Pass QdrantUrl explicitly for an operational ranked-search build. Without it, Raw/Decoded tools remain
possible but ranked operations return MCP_QDRANT_URL_REQUIRED. There is no guessed endpoint default.
The actual launcher accepts required absolute --data-root, optional --qdrant-url and optional --daemon;
the package relies on the two sibling binaries instead of embedding a daemon override. Source-info annotations
default on in MCP query schemas; there is no source-info launcher flag or public source_info RPC.

The [builder](scripts/build-packages.ps1) reads inputs/templates and writes only its own fresh output. It never
opens, copies or initializes DataRoot, indexes sources, starts Qdrant or an MCP service, edits host configuration,
installs a client package, performs auth/UI actions, or recursively deletes anything. Its only binary invocations
are the three bounded metadata-only --version probes. Failure preserves partial output for inspection; choose a
new output directory for a subsequent attempt. Never use an application root as packaging output.

It copies all three binaries by value, compares source/copy SHA256, reopens each archive and checks every member's
bytes/hash, and checks that inputs/copies stayed unchanged. Canonical SKILL.md is not rewritten.

## Outputs and native mapping

~~~text
OutputDirectory/
  plugin/
    plugin.json
    mcp.json
    bin/eliot-search-mcp.exe
    bin/eliot-searchd.exe
    bin/eliot-search.exe
    skills/eliot-search/SKILL.md
  desktop/manifest.json
  eliot-search.mcpb
  eliot-search-skill.zip
~~~

The plugin manifest uses the [Agent Plugins format](https://developers.openai.com/plugins/build/plugins).
The generated mcp.json has an absolute command targeting its own packaged plugin/bin binary. It is
MACHINE-BOUND to this output location and DataRoot/URL, not a relocatable plugin ZIP. Keep that plugin folder
as the installed binary location. If a native plugin installer copies metadata into its cache, the absolute
command still targets this retained folder. Moving/deleting it breaks that binding: rebuild into the intended
fresh location and register the new mapping. No universal plugin-root macro is assumed.

The [MCP template](mcp.template.json) contains unresolved build placeholders and must never be registered as-is.
Native client registration should reference the generated packaged binary and its rendered argv. Use native
add/update commands where supported; preserve other servers, profiles and skill overrides. Codex portable
mcp.json is not an OpenCode, Claude or Antigravity config file to copy wholesale.

The [Desktop template](desktop/manifest.template.json) uses
[MCPB 0.3 binary format](https://github.com/modelcontextprotocol/mcpb/blob/main/MANIFEST.md).
The .mcpb has exactly manifest.json plus server/eliot-search-mcp.exe and sibling server/eliot-searchd.exe.
Only its executable path uses the documented MCPB extension-directory substitution. DataRoot and optional
Qdrant URL are literal arguments, so this package binds that local dataset/endpoint. It contains no dataset
objects, CONTROL, credentials, Qdrant storage, Node/Python service or runtime installer.

## Operator commands outside the checkout

The local package includes eliot-search.exe for initialization, Index and optional foreground watch; the MCPB
contains only the two query siblings. Run operator commands explicitly against the intended dataset. These
examples are instructions for later authorized use, not actions performed by the builder:

~~~powershell
$operator = 'C:\example\agent-packages\eliot-search-operator-root\plugin\bin\eliot-search.exe'
$packagedDaemon = 'C:\example\agent-packages\eliot-search-operator-root\plugin\bin\eliot-searchd.exe'
& $operator initialize-data-root --data-root 'C:\example\search-data' --daemon $packagedDaemon
& $operator index 'C:\example\repo' --data-root 'C:\example\search-data' --qdrant-url 'http://127.0.0.1:6334' --daemon $packagedDaemon
& $operator watch --data-root 'C:\example\search-data' --qdrant-url 'http://127.0.0.1:6334' --daemon $packagedDaemon
~~~

Initialize once for a new root; do not initialize an existing one. Qdrant is operator-started and separately
owned. Watch is optional foreground work, not an OS service; ROOT_BUSY during an active refresh is expected,
while watch idle releases the root. MCP windows also retain the root until closed/expired. See the
[operator guide](../../QUICKSTART.md#index-and-search). Explicit --daemon prevents an existing environment
override from selecting another daemon without modifying that profile. No global PATH or checkout working
directory is required.

## Skill installation is separate

The Skill ZIP contains exactly eliot-search/SKILL.md and no executables.

- Codex: native user/project .agents/skills, or an enabled plugin's skills component.
- Claude Code: native user/project .claude/skills, or enabled plugin skills.
- OpenCode: native .config/opencode/skills or project .opencode/skills; respect the installed V1/V2 config schema.
- Antigravity: current global .gemini/config/skills or project .agents/skills.
- Claude Desktop/Cowork account Skills: upload the Skill ZIP through Customize > Skills and enable it for the
  authenticated account, subject to account/org policy. A local .claude/skills file is not Desktop/Cowork proof.

See [Claude native Skill scope](https://code.claude.com/docs/en/skills#use-skills-in-cowork-and-cloud-sessions)
and [account upload](https://support.claude.com/en/articles/12512180-use-skills-in-claude).
Desktop supports the ordinary local mcpServers config route for an existing binary; it supplied the original
native proof below. The current Desktop route is the subsequently installed .mcpb extension. Uploading a Skill neither
registers the MCP server nor makes local stdio a cloud connector.

Artifact creation, native configuration/enablement, Skill discovery/use and actual MCP work are separate
outcomes. All five recorded native scopes are accepted in [AD1](../../docs/audit/ANALYSIS_REGISTRY.md#agent-delivery).
The unchanged canonical Skill SHA256 is 3DC3E8F73AD6E827DC1A3EF6C3208875BC593CE599F678947D73AE4D86D1242C.
Four native coding-client copies and Desktop account upload/enablement (by you, v1) are retained; actual native
chats selected the account Skill. Existing custom Skills and client/profile entries were preserved.

The current verified version 0.0.0 shared output is
C:\Users\kleym\.eliot-search\agent-tools-20261108\package-v4; its actual command is
plugin/bin/eliot-search-mcp.exe, alongside searchd and the operator CLI. MCP SHA256 is
9ECA3FBE804F93584C458909C00725F4EE46AEBE62099E6C351329144DD4730A. CLI/daemon are unchanged v3 copies.
The real v4 builder exited 0; trio pins, nine files, archive members and three published schemas passed.
DataRoot is C:\Users\kleym\.eliot-search\agent-tools-20261108\corpus-search and URL http://127.0.0.1:7637.
Build/package proof is under
C:\Users\kleym\.eliot-search\agent-tools-20261108\logs\desktop-native-20261108\run-002-v4-inline:
package-build.json and package-verification.json. Packaging did not open/initialize/copy the dataset.

Main ed8ce5b4 changes only Open's self-contained schema/description. Sol SOURCE PASS, frozen checks and resolved
schema-equivalence proof are under
C:\Development\Rust\targets\task-agent-mcp-build\run-20261108-v4-self-contained-open.
DTO/parser/handlers and the other six tools are unchanged. Candidate3 selected SDK native cleanup/continuation
PASS remains at its original scope. Codex 0.161, Claude Code 2.1.278, signed AGY CLI 1.5.3 and OpenCode 2.0.32
retain their accepted v3 native Skill/tool/context/answer/Close flows, with their original exit/custody proofs.
All four current command-only bindings target v4; those model flows were not replayed on v4.

Desktop 4.23118.0 / Opus 5.5 Extra has separate actual v4 normal-Chat proof using the enabled native account
Skill, initially through official mcpServers.eliot-search configuration. After explicit human approval on
2026-11-09, the current v4 MCPB was installed/enabled through native Extensions. Its actual server is
C:\Users\kleym\AppData\Roaming\Claude\Claude Extensions\local.mcpb.eliot-search.eliot-search\server\eliot-search-mcp.exe
with the unchanged root/URL above and matching MCP/daemon pins. Seven tools were discovered; one native
Search (limit 3), Open, zero-extra-context Expand and actual valid-handle Close returned saved Rust text,
both ObjectRefs, [0,6405) unit context and not_checked. Only the duplicate manual mcpServers.eliot-search
entry was removed; github/codebase_memory, other fields, ACL and account Skill were preserved.
The initial string-shaped Open rejection, recovered numeric-handle refusal and native answer-only corrections
remain preserved. The larger code block is explicitly abridged; the short signature is literal. The one exact
literal match is scoped to the registered g1 inventory, not semantic declaration uniqueness.

The optional v4 MCPB SHA256 is 11707D89CF0D9AFCD18384773F71A2794329FC5E93CC8F556A03B2BDA41175C3.
It is installed and enabled in the recorded Desktop scope. Its command uses the documented extension-directory
macro with literal root/URL. The earlier withdrawn request and unpressed v3 review remain historical;
installation followed the new explicit approval for v4, with no Cowork claim. Bounded proof is under
C:\Users\kleym\.eliot-search\agent-tools-20261108\logs\desktop-extension-20261109\run-001:
native-answer-ui.txt, native-close-ui.txt, installed-process-binding.json and config-after-native.json.
The old shared-v3 and AppData/Codex-cache packages remain preserved;
all eighteen old package files were unchanged. Operational paths do not depend on the invisible AppData alias.

The shared g1 snapshot has 361 registered sources and zero preparation gaps, not a whole-repository/live-file
coverage guarantee. Saved paths remain historical with live_status:not_checked; CONTROL lag is not a live-file
audit. Backend storage remains C:\Users\kleym\.es8\q\{s,sn,tmp}, with operator files under .es8\operator and
HTTP/gRPC 7636/7637. App/backend were left working; the final read-only readyz/TCP observation passed.
No service/autostart or lifetime across whole Codex/runtime exit/reboot was created or verified.
AD1 is DONE for the recorded five-client scopes after Sol DESKTOP_NATIVE_ACCEPTED.

Reproduce native client work with a fresh session loading the installed Skill/MCP and the ready shared root.
The recorded runs used existing authenticated native client/provider access.
Recorded Codex exec uses --sandbox read-only --ephemeral with per-run MCP overrides. Claude Code uses
--strict-mcp-config with a temporary search-only config, --tools Skill and explicit --allowedTools
Skill(eliot-search) plus mcp__eliot-search__*, with --permission-mode dontAsk --permission-prompts none.
Claude's captured model is claude-sonnet-5; exact Codex/AGY model IDs were not captured, so their observed
admitted native sessions do not establish arbitrary-model compatibility. AGY headless needs session-only
--settings permissions.allow entries for the seven exact mcp(eliot-search/tool) rules.

Qualified OpenCode 2.0.32 uses run --standalone --model openrouter/nvidia/nemotron-3-ultra-550b-a55b:free.
The fresh owned working directory's .opencode/opencode.jsonc sets
providers.openrouter.models["nvidia/nemotron-3-ultra-550b-a55b:free"].limit.output to 4096 for that run.
CodeMode execute.code must contain plain JavaScript using names from the native catalog. Actual ranked Search,
retained Open/Expand, successful Close and grounded answer passed with exit 0 and ROOT_RELEASED.
Its effective answer is native 001 plus the 002 unit-span correction and 003 retraction, each preserved:
catch strings do not establish MCP error reasons/order/counts; raw MCP responses are authoritative.
Sol accepted the matched quotations/refs/ranges. Recovered argument errors, the first Liquid CodeMode failure
and both paid Sol HTTP 402 pre-admission failures remain preserved. This is the selected free-model scope.

Desktop's qualified invocation is normal Chat selecting the native account Skill and configured local MCP.
Use a fresh chat after normal metadata reload; public tool schemas define arguments and returned opaque handles
define windows. The saved seven-tool SDK catalog and actual native calls are separate from full host-schema
fields, which the UI did not expose. No arbitrary model/UI variant is qualified by these observations.

Use the exact command/proof artifacts linked from AD1, keep permissions/model caps session-scoped, preserve
unrelated global settings, and close each retained window promptly. The qualified free-model run reused the
existing provider/auth and made no billing change.
