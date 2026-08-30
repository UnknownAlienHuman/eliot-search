[CmdletBinding()]
param(
    [string]$Root = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path,
    [switch]$Json
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

$errors = [System.Collections.Generic.List[string]]::new()

function Fail([string]$Message) {
    $script:errors.Add($Message)
}

function Read-File([string]$Path) {
    $full = Join-Path $Root $Path
    if (-not (Test-Path $full -PathType Leaf)) {
        Fail "Missing file: $Path"
        return ''
    }
    [IO.File]::ReadAllText($full)
}

function Value([string]$Text, [string]$Key, [bool]$Required = $true) {
    $match = [regex]::Match($Text, ('(?m)^{0}\s*=\s*"([^"]*)"\s*$' -f [regex]::Escape($Key)))
    if (-not $match.Success) {
        if ($Required) { Fail "Missing string: $Key" }
        return ''
    }
    $match.Groups[1].Value
}

function Number([string]$Text, [string]$Key) {
    $match = [regex]::Match($Text, ('(?m)^{0}\s*=\s*(-?\d+)\s*$' -f [regex]::Escape($Key)))
    if (-not $match.Success) {
        Fail "Missing integer: $Key"
        return [int64]0
    }
    [int64]$match.Groups[1].Value
}

function Flag([string]$Text, [string]$Key) {
    $match = [regex]::Match($Text, ('(?m)^{0}\s*=\s*(true|false)\s*$' -f [regex]::Escape($Key)))
    if (-not $match.Success) {
        Fail "Missing boolean: $Key"
        return $false
    }
    $match.Groups[1].Value -eq 'true'
}

function Array([string]$Text, [string]$Key) {
    $match = [regex]::Match($Text, ('(?ms)^{0}\s*=\s*\[(.*?)\]' -f [regex]::Escape($Key)))
    if (-not $match.Success) { return @() }
    @(
        [regex]::Matches($match.Groups[1].Value, '"([^"\r\n]+)"') |
            ForEach-Object { $_.Groups[1].Value }
    )
}

function Section([string]$Text, [string]$Name) {
    $match = [regex]::Match($Text, ('(?ms)^\[{0}\]\s*(.*?)(?=^\[|\z)' -f [regex]::Escape($Name)))
    if (-not $match.Success) {
        Fail "Missing section: $Name"
        return ''
    }
    $match.Groups[1].Value
}

function Same([object[]]$Left, [object[]]$Right) {
    $a = @($Left | ForEach-Object { [string]$_ } | Sort-Object -Unique)
    $b = @($Right | ForEach-Object { [string]$_ } | Sort-Object -Unique)
    if ($a.Count -ne $b.Count) { return $false }
    for ($i = 0; $i -lt $a.Count; $i++) {
        if ($a[$i] -cne $b[$i]) { return $false }
    }
    $true
}

function Same-Sequence([object[]]$Left, [object[]]$Right) {
    $a = @($Left | ForEach-Object { [string]$_ })
    $b = @($Right | ForEach-Object { [string]$_ })
    if ($a.Count -ne $b.Count) { return $false }
    for ($i = 0; $i -lt $a.Count; $i++) {
        if ($a[$i] -cne $b[$i]) { return $false }
    }
    $true
}

function Empty-ControlDir([string]$Path) {
    $full = Join-Path $Root $Path
    if (-not (Test-Path $full -PathType Container)) {
        Fail "Missing directory: $Path"
        return
    }
    foreach ($file in @(Get-ChildItem $full -Recurse -File)) {
        if ($file.Name -notin @('README.md', '.gitkeep')) {
            Fail "Premature control record: $($file.FullName.Substring($Root.Length + 1))"
        }
    }
}

$ticketManifest = Read-File 'swarm/ticket-drafts/manifest.toml'
$contextManifest = Read-File 'swarm/context-drafts/manifest.toml'
$p00Manifest = Read-File 'docs/contracts/p00/manifest.toml'
$orchestration = Read-File 'swarm/orchestration.toml'
$launch = Read-File 'swarm/launch-state.toml'

if ((Value $ticketManifest 'status') -cne 'DRAFT_ONLY_NOT_ISSUED' -or (Number $ticketManifest 'draft_count') -ne 3) {
    Fail 'Ticket draft manifest identity/count mismatch.'
}
foreach ($zero in @('issued_ticket_count', 'active_lease_count', 'submission_count', 'accepted_review_count', 'package_handoff_count', 'wave_receipt_count')) {
    if ((Number $ticketManifest $zero) -ne 0) { Fail "$zero must be zero." }
}
if ((Number $contextManifest 'schema_version') -ne 2 -or (Value $contextManifest 'status') -cne 'NON_CLAIMABLE_CONTEXT_DRAFTS' -or (Number $contextManifest 'draft_count') -ne 3 -or (Number $contextManifest 'materialized_context_count') -ne 0) {
    Fail 'Context draft manifest identity/count mismatch.'
}
if ((Number $contextManifest 'writer_visible_artifact_count_per_context') -ne 1) {
    Fail 'Each materialized context must be one artifact.'
}

$ordinarySourceCeiling = [int](Number $contextManifest 'ordinary_static_source_file_ceiling')
$p00SourceCeiling = [int](Number $contextManifest 'p00_exact_contract_pack_source_file_ceiling')
$exceptionPackages = @(Array $contextManifest 'p00_exact_contract_pack_exception_packages')
$fragmentCeiling = [int](Number $contextManifest 'max_registry_fragments_per_context')
$handoffCeiling = [int](Number $contextManifest 'max_accepted_handoff_slots_per_context')
if ($ordinarySourceCeiling -ne 16 -or $p00SourceCeiling -ne 24) {
    Fail 'Context source ceilings must remain ordinary=16 and P00 exact-pack=24.'
}
if (-not (Same $exceptionPackages @('search-contracts'))) {
    Fail 'Only search-contracts may use the P00 exact-contract-pack source exception.'
}
if ($fragmentCeiling -ne 6 -or $handoffCeiling -ne 1) {
    Fail 'Context fragment/handoff ceilings must remain 6/1.'
}

$expected = [ordered]@{
    'search-contracts' = @{ Launch = 'AUTHORIZED'; Precondition = 'CURRENTLY_PRESENT'; Scope = 'crates/search-contracts/**'; Soft = 8000; Handoffs = 0; CeilingClass = 'P00_EXACT_CONTRACT_PACK' }
    'search-domain' = @{ Launch = 'CONDITIONAL'; Precondition = 'ACCEPTED_SEARCH_CONTRACTS_HANDOFF_REQUIRED'; Scope = 'crates/search-domain/**'; Soft = 7000; Handoffs = 1; CeilingClass = 'ORDINARY' }
    'search-ports' = @{ Launch = 'CONDITIONAL'; Precondition = 'ACCEPTED_SEARCH_CONTRACTS_HANDOFF_REQUIRED'; Scope = 'crates/search-ports/**'; Soft = 5500; Handoffs = 1; CeilingClass = 'ORDINARY' }
}

$manifestPackages = @(
    [regex]::Matches($ticketManifest, '(?m)^package\s*=\s*"([^"]+)"\s*$') |
        ForEach-Object { $_.Groups[1].Value }
)
$contextDraftBlocks = [regex]::Split($contextManifest, '(?m)^\[\[draft\]\]\s*$')
$contextPackages = [System.Collections.Generic.List[string]]::new()
$contextCeilingClasses = @{}
for ($i = 1; $i -lt $contextDraftBlocks.Count; $i++) {
    $package = Value $contextDraftBlocks[$i] 'package'
    if ([string]::IsNullOrWhiteSpace($package)) { continue }
    if ($contextCeilingClasses.ContainsKey($package)) {
        Fail "Duplicate context draft manifest package: $package"
        continue
    }
    [void]$contextPackages.Add($package)
    $contextCeilingClasses[$package] = Value $contextDraftBlocks[$i] 'source_ceiling_class'
}
if (-not (Same $manifestPackages @($expected.Keys)) -or -not (Same $contextPackages.ToArray() @($expected.Keys))) {
    Fail 'Draft package set mismatch.'
}

$p00RequiredPaths = @(
    Array $p00Manifest 'required_files' |
        ForEach-Object { "docs/contracts/p00/$_" }
)
if ($p00RequiredPaths.Count -ne 12) {
    Fail 'P00 manifest required-file count must remain 12.'
}
$searchContractsFixedSources = @(
    'AGENTS.md',
    'crates/search-contracts/AGENTS.md',
    'docs/handoff/AUTHORITY_MAP.md',
    'swarm/ASSIGNMENT_PROTOCOL.md',
    'swarm/assignments/search-contracts.md',
    'docs/handoff/P00_BOOTSTRAP.md'
)
$expectedSearchContractsSources = @(
    $searchContractsFixedSources +
    $p00RequiredPaths[0] +
    'docs/contracts/p00/manifest.toml' +
    $p00RequiredPaths[1..($p00RequiredPaths.Count - 1)]
)

foreach ($entry in $expected.GetEnumerator()) {
    $package = [string]$entry.Key
    $spec = $entry.Value
    $ticketPath = "swarm/ticket-drafts/p00/$package.toml"
    $contextPath = "swarm/context-drafts/p00/$package.toml"
    $ticket = Read-File $ticketPath
    $context = Read-File $contextPath

    if ((Value $ticket 'record_kind') -cne 'assignment_ticket_draft' -or (Value $ticket 'status') -cne 'DRAFT_ONLY_NOT_ISSUED') {
        Fail "$package ticket is not a draft."
    }
    foreach ($unsafe in @('claimable', 'authorizes_implementation', 'creates_lease', 'may_be_writer_acknowledged')) {
        if (Flag $ticket $unsafe) { Fail "$package ticket enables $unsafe." }
    }
    if ((Value $ticket 'package') -cne $package -or (Value $ticket 'stage') -cne 'W0' -or (Value $ticket 'phase') -cne 'P00' -or (Number $ticket 'wave') -ne 0) {
        Fail "$package ticket stage identity mismatch."
    }
    if ((Value $ticket 'launch_class') -cne $spec.Launch -or (Value $ticket 'launch_precondition') -cne $spec.Precondition) {
        Fail "$package launch classification mismatch."
    }

    $identity = Section $ticket 'unresolved_identity'
    foreach ($key in @('ticket_id', 'lease_id', 'writer', 'reviewer')) {
        if ((Value $identity $key) -cne 'UNASSIGNED') { Fail "$package $key is prematurely assigned." }
    }
    foreach ($key in @('base_commit', 'branch_or_worktree')) {
        if ((Value $identity $key) -cne 'UNSELECTED') { Fail "$package $key is prematurely selected." }
    }
    if ((Value $identity 'ticket_canonical_digest') -cne 'UNAVAILABLE') {
        Fail "$package ticket digest is prematurely available."
    }

    $fence = Section $ticket 'repository_fence'
    if ((Value $fence 'repository') -cne 'UnknownAlienHuman/eliot-search' -or (Value $fence 'write_scope') -cne $spec.Scope) {
        Fail "$package repository fence mismatch."
    }
    $limits = Section $ticket 'limits'
    if ((Number $limits 'soft_src_lines') -ne $spec.Soft -or (Number $limits 'hard_total_lines') -ne 10000 -or -not (Flag $limits 'one_active_writer')) {
        Fail "$package limits mismatch."
    }

    $deps = Section $ticket 'dependencies'
    $requiredDeps = @(Array $deps 'required_handoff_packages')
    $acceptedDeps = @(Array $deps 'accepted_handoff_refs')
    if ($spec.Handoffs -eq 0) {
        if ($requiredDeps.Count -ne 0 -or $acceptedDeps.Count -ne 0 -or (Value $deps 'status') -cne 'NOT_REQUIRED') {
            Fail "$package must have no dependency handoff."
        }
    }
    else {
        if (-not (Same $requiredDeps @('search-contracts')) -or $acceptedDeps.Count -ne 0 -or (Value $deps 'status') -cne 'UNAVAILABLE') {
            Fail "$package must remain blocked on search-contracts handoff."
        }
    }

    if ((Value $context 'record_kind') -cne 'writer_context_draft' -or (Value $context 'status') -cne 'UNMATERIALIZED_DRAFT') {
        Fail "$package context is not an unmaterialized draft."
    }
    foreach ($unsafe in @('claimable', 'authorizes_implementation')) {
        if (Flag $context $unsafe) { Fail "$package context enables $unsafe." }
    }
    if ((Value $context 'package') -cne $package -or (Value $context 'base_commit') -cne 'UNSELECTED' -or (Value $context 'materialized_context_ref') -cne 'UNAVAILABLE' -or (Value $context 'materialized_context_sha256') -cne 'UNAVAILABLE') {
        Fail "$package context is prematurely materialized."
    }
    if ((Value $context 'materialization_mode') -cne 'canonical_concatenated_bundle' -or (Number $context 'writer_visible_artifact_count') -ne 1) {
        Fail "$package context materialization contract mismatch."
    }
    if ([string]$contextCeilingClasses[$package] -cne [string]$spec.CeilingClass) {
        Fail "$package context ceiling class mismatch."
    }

    $sources = @(Array $context 'source_files')
    $fragments = @(Array $context 'registry_fragments')
    $handoffs = @(Array $context 'accepted_handoff_slots')
    if ((Number $context 'source_file_count') -ne $sources.Count -or (Number $context 'registry_fragment_count') -ne $fragments.Count -or (Number $context 'accepted_handoff_slot_count') -ne $handoffs.Count) {
        Fail "$package context counts mismatch."
    }

    $sourceCeiling = if ($exceptionPackages -contains $package) { $p00SourceCeiling } else { $ordinarySourceCeiling }
    if ($sources.Count -gt $sourceCeiling) {
        Fail "$package context source count $($sources.Count) exceeds ceiling $sourceCeiling."
    }
    if ($fragments.Count -gt $fragmentCeiling) {
        Fail "$package context fragment count $($fragments.Count) exceeds ceiling $fragmentCeiling."
    }
    if ($handoffs.Count -gt $handoffCeiling) {
        Fail "$package context handoff slot count $($handoffs.Count) exceeds ceiling $handoffCeiling."
    }
    if ($package -eq 'search-contracts' -and -not (Same-Sequence $sources $expectedSearchContractsSources)) {
        Fail 'search-contracts P00 exception must equal the exact manifest-closed contract pack and fixed integration sources in canonical order.'
    }

    foreach ($required in @(
        'AGENTS.md',
        "crates/$package/AGENTS.md",
        'docs/handoff/AUTHORITY_MAP.md',
        'swarm/ASSIGNMENT_PROTOCOL.md',
        "swarm/assignments/$package.md",
        'docs/handoff/P00_BOOTSTRAP.md',
        'docs/contracts/p00/manifest.toml'
    )) {
        if ($sources -notcontains $required) { Fail "$package context omits $required." }
    }

    foreach ($source in $sources) {
        if (-not (Test-Path (Join-Path $Root $source) -PathType Leaf)) {
            Fail "$package context source missing: $source"
        }
        if ($source -like 'docs/architecture/*' -or $source -match '^(crates|bins)/.+/src/') {
            Fail "$package context includes forbidden source: $source"
        }
    }

    foreach ($selector in @(
        "swarm/crates.toml::package[name=$package]",
        "swarm/function-packets.toml::foundation[package=$package]",
        'swarm/stages.toml::stage[id=W0]'
    )) {
        if ($fragments -notcontains $selector) { Fail "$package context omits selector $selector." }
    }

    if ($spec.Handoffs -eq 0) {
        if ($handoffs.Count -ne 0) { Fail 'search-contracts context must have no accepted handoff slot.' }
    }
    elseif (-not (Same $handoffs @('search-contracts::accepted_package_and_api_handoff'))) {
        Fail "$package handoff slot mismatch."
    }
}

if ((Number $orchestration 'schema_version') -ne 5) {
    Fail 'Orchestration schema_version must be 5.'
}
foreach ($pair in @(
    @('ticket_draft_manifest', 'swarm/ticket-drafts/manifest.toml'),
    @('context_draft_manifest', 'swarm/context-drafts/manifest.toml'),
    @('control_plane_schema_registry', 'swarm/control-plane-schema.toml'),
    @('control_plane_type_registry', 'swarm/schemas/types-v1.toml')
)) {
    if ((Value $orchestration $pair[0]) -cne $pair[1]) {
        Fail "Orchestration path mismatch: $($pair[0])"
    }
}
if ((Value $orchestration 'workflow_policy') -cne 'manual_only') {
    Fail 'Orchestration workflow policy must remain manual_only.'
}

if ((Value $launch 'active_stage') -cne 'P00' -or (Number $launch 'active_wave') -ne 0) {
    Fail 'Launch state must remain P00/W0.'
}
if ((Number $launch 'orchestration_registry_schema_version') -ne 5 -or (Value $launch 'orchestration_registry_path') -cne 'swarm/orchestration.toml') {
    Fail 'Launch state must pin orchestration schema v5.'
}
if (-not (Same @(Array $launch 'authorized_packages') @('search-contracts'))) {
    Fail 'Only search-contracts may be authorized.'
}
if (-not (Same @(Array $launch 'conditional_packages') @('search-domain', 'search-ports'))) {
    Fail 'Conditional package set mismatch.'
}

foreach ($directory in @(
    'swarm/tickets',
    'swarm/context-manifests',
    'swarm/leases',
    'swarm/submissions',
    'swarm/reviews',
    'swarm/handoffs',
    'swarm/supersessions',
    'swarm/wave-receipts'
)) {
    Empty-ControlDir $directory
}

$forbiddenWorkflowTriggers = '(?m)^\s{0,6}(push|pull_request|pull_request_target|merge_group|schedule|workflow_run|repository_dispatch|workflow_call):\s*$'
$workflowFiles = @(
    Get-ChildItem (Join-Path $Root '.github/workflows') -File |
        Where-Object { $_.Extension -in @('.yml', '.yaml') }
)
foreach ($file in $workflowFiles) {
    $body = [IO.File]::ReadAllText($file.FullName)
    if ([regex]::IsMatch($body, $forbiddenWorkflowTriggers) -or $body.IndexOf('workflow_dispatch:', [StringComparison]::Ordinal) -lt 0) {
        Fail "Workflow is not manual-only: $($file.Name)"
    }
    if (-not [regex]::IsMatch($body, '(?m)^\s{2}contents:\s*read\s*$')) {
        Fail "Workflow is not read-only: $($file.Name)"
    }
    if ($body.IndexOf('persist-credentials: false', [StringComparison]::Ordinal) -lt 0) {
        Fail "Workflow persists checkout credentials: $($file.Name)"
    }
}

$result = [ordered]@{
    ok = ($errors.Count -eq 0)
    ticket_drafts = 3
    context_drafts = 3
    ordinary_source_ceiling = $ordinarySourceCeiling
    p00_exception_source_ceiling = $p00SourceCeiling
    p00_exception_packages = $exceptionPackages
    issued_tickets = 0
    active_leases = 0
    workflows = $workflowFiles.Count
    launch = 'P00/W0'
    errors = @($errors)
}

if ($Json) {
    $result | ConvertTo-Json -Depth 6
}
else {
    Write-Host "P00 draft control validation: drafts=3 contexts=3 ordinary=$ordinarySourceCeiling exception=$p00SourceCeiling issued=0 leases=0 workflows=$($result.workflows) launch=P00/W0"
    foreach ($error in $errors) {
        Write-Host "ERROR: $error" -ForegroundColor Red
    }
    if ($result.ok) { Write-Host 'PASS' -ForegroundColor Green }
}

if (-not $result.ok) { exit 1 }
