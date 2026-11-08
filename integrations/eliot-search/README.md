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
Local MCP installation is independently available through the Desktop Extensions UI with the .mcpb;
uploading a Skill does not register that MCP server or make local stdio a cloud connector.

Artifact creation, native installation/enablement, skill discovery/use, and MCP initialize/tools-list/tool-call
are separate outcomes. Four canonical coding-client Skill copies are installed; the current Codex host catalog
discovers eliot-search. After a user-reported UI state change, Claude Desktop account upload/enablement was
observed as by you, v1, with one SKILL.md and enabled switch. Existing custom Skills were preserved. The earlier
DWM preflight failure is historical; it does not describe the current accessible UI. Desktop MCPB software
installation awaits action-time approval and is NOT_RUN. The four owned coding-client MCP entries now use the
shared package/data-root. Codex 0.161, Claude Code 2.1.278 and signed AGY CLI 1.5.3 passed native Skill/tool
work, retained context/answer and Close/exit 0. AGY's actual wait/both EOF/root-release proof is retained.
OpenCode 2.0.32 passed selected native Search/retained context/Close/answer on the audited free model below.
Only Desktop MCP install/enablement/initialize/discovery and retained context/answer remain in AD1; no Desktop
runtime follows from the four coding-client results.

Candidate3 has CORE/MCP SOURCE PASS with all reviewed P2 items closed and locked check/strict Clippy/Release PASS.
The real builder exited 0 using the matching version 0.0.0 trio at
C:\Development\Rust\targets\task-agent-mcp-build\run-20261108-v3-cancel-delivery\release-checked.
Its current verified shared output is C:\Users\kleym\.eliot-search\agent-tools-20261108\package:
the actual command is plugin/bin/eliot-search-mcp.exe, alongside searchd and the operator CLI.
All three pins/copies, nine output files, archive members and published schemas passed; source-freeze hashes
remained unchanged. DataRoot is C:\Users\kleym\.eliot-search\agent-tools-20261108\corpus-search and Qdrant URL
is http://127.0.0.1:7637. Desktop keeps only the documented extension-directory macro for its binary; root/URL
are literals, with no user_config. The actual Claude chooser accepted this shared MCPB and opened its review
dialog; a fresh read-only observation during the client runs still showed Install pending. No input/query
action was taken. The UI warns of access to everything on the computer, not a dataset sandbox.

The first AppData/Codex MSIX-cache package is preserved as historical evidence, not the operational install.
Its short AppData alias was invisible to Claude; all nine old package files remain byte-identical. Operational
binary/data/backend paths must not depend on that alias or Codex cache. Selected backend locations are
C:\Users\kleym\.es8\q\{s,sn,tmp}, with operator files under C:\Users\kleym\.es8\operator and HTTP/gRPC 7636/7637.
This is an operator-location correction, not a Rust, database or protocol change.

Sol accepted candidate3 SELECTED_NATIVE_PASS including unread-output cleanup, full 1+63 window continuation
and three stale-handle refusals. Packaging itself did not open/initialize the dataset or register/install
anything. The actual-repository shared root is READY: generation 1, 361 sources, zero preparation gaps.
The operator-owned backend remains intentionally live for client verification; no service/autostart is added.
See [AD1 evidence](../../docs/audit/ANALYSIS_REGISTRY.md#agent-delivery) for exact shared root/package/pins,
client outcomes and preserved failed attempts. AD1 remains IN_PROGRESS.

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
retained Open/Expand, successful Close and grounded answer passed with exit 0 and ROOT_RELEASED. Two recovered
argument errors, ordinary Busy/redundant Close refusals, the first Liquid CodeMode failure, and both paid Sol
HTTP 402 pre-admission failures remain preserved. This is the selected free-model invocation scope.

Use the exact command/proof artifacts linked from AD1, keep permissions/model caps session-scoped, preserve
unrelated global settings, and close each retained window promptly. The qualified free-model run reused the
existing provider/auth and made no billing change.
