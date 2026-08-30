[CmdletBinding()]
param(
    [string]$Root = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path,
    [switch]$Json
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

$errors = [System.Collections.Generic.List[string]]::new()
$warnings = [System.Collections.Generic.List[string]]::new()
function Add-Error([string]$Message) { $script:errors.Add($Message) }
function Add-Warning([string]$Message) { $script:warnings.Add($Message) }
function Read-Required([string]$RelativePath) {
    $path = Join-Path $Root $RelativePath
    if (-not (Test-Path $path -PathType Leaf)) {
        Add-Error "Missing required file: $RelativePath"
        return ''
    }
    [IO.File]::ReadAllText($path)
}
function TStr([string]$Text, [string]$Key, [bool]$Required = $true) {
    $match = [regex]::Match($Text, ('(?m)^{0}\s*=\s*"([^"]*)"\s*$' -f [regex]::Escape($Key)))
    if (-not $match.Success) {
        if ($Required) { Add-Error "Missing TOML string '$Key'." }
        return ''
    }
    $match.Groups[1].Value
}
function TInt([string]$Text, [string]$Key, [bool]$Required = $true) {
    $match = [regex]::Match($Text, ('(?m)^{0}\s*=\s*(\d+)\s*$' -f [regex]::Escape($Key)))
    if (-not $match.Success) {
        if ($Required) { Add-Error "Missing TOML integer '$Key'." }
        return [int64]0
    }
    [int64]$match.Groups[1].Value
}
function TBool([string]$Text, [string]$Key, [bool]$Required = $true) {
    $match = [regex]::Match($Text, ('(?m)^{0}\s*=\s*(true|false)\s*$' -f [regex]::Escape($Key)))
    if (-not $match.Success) {
        if ($Required) { Add-Error "Missing TOML boolean '$Key'." }
        return $false
    }
    $match.Groups[1].Value -eq 'true'
}
function TArray([string]$Text, [string]$Key) {
    $match = [regex]::Match($Text, ('(?ms)^{0}\s*=\s*\[(.*?)\]' -f [regex]::Escape($Key)))
    if (-not $match.Success) { return @() }
    @([regex]::Matches($match.Groups[1].Value, '"([^"\r\n]+)"') | ForEach-Object { $_.Groups[1].Value })
}
function Same-Set([string[]]$Left, [string[]]$Right) {
    $a = @($Left | Sort-Object -Unique)
    $b = @($Right | Sort-Object -Unique)
    if ($a.Count -ne $b.Count) { return $false }
    for ($i = 0; $i -lt $a.Count; $i++) {
        if ($a[$i] -cne $b[$i]) { return $false }
    }
    $true
}
function Unique-Strings([string[]]$Values) { @($Values | Sort-Object -Unique) }
function Validate-File([string]$Owner, [string]$RelativePath, [string]$Kind) {
    if ([string]::IsNullOrWhiteSpace($RelativePath)) { return }
    if ($RelativePath.StartsWith('docs/architecture/', [StringComparison]::Ordinal)) {
        Add-Error "$Owner declares forbidden ordinary architecture read: $RelativePath"
    }
    if (-not (Test-Path (Join-Path $Root $RelativePath) -PathType Leaf)) {
        Add-Error "$Owner references missing $Kind file: $RelativePath"
    }
}
function Require-Tokens([string]$RelativePath, [string]$Text, [string[]]$Tokens) {
    foreach ($token in $Tokens) {
        if ($Text.IndexOf($token, [StringComparison]::Ordinal) -lt 0) {
            Add-Error "$RelativePath is missing required token: $token"
        }
    }
}

$paths = [ordered]@{
    packages = 'swarm/crates.toml'
    functions = 'swarm/function-packets.toml'
    stages = 'swarm/stages.toml'
    readsets = 'swarm/stage-readsets.toml'
    gates = 'swarm/gates.toml'
    launch = 'swarm/launch-state.toml'
    agents = 'AGENTS.md'
    authority = 'docs/handoff/AUTHORITY_MAP.md'
    assignment_protocol = 'swarm/ASSIGNMENT_PROTOCOL.md'
    handoff = 'docs/handoff/SWARM_STAGE_READSETS.md'
    swarm_readme = 'swarm/README.md'
    tools_readme = 'tools/README.md'
    w8_client = 'bins/eliot-search/W8_CLIENT.md'
    w10_eval = 'crates/search-eval/W10_OPTIONAL_EVALUATION.md'
    workflow = '.github/workflows/stage-readsets.yml'
}
$text = [ordered]@{}
foreach ($entry in $paths.GetEnumerator()) { $text[$entry.Key] = Read-Required $entry.Value }

# Package registry.
$packageBlocks = [regex]::Split($text.packages, '(?m)^\[\[package\]\]\s*$')
$packages = [ordered]@{}
for ($i = 1; $i -lt $packageBlocks.Count; $i++) {
    $block = $packageBlocks[$i]
    $name = TStr $block 'name'
    if ([string]::IsNullOrWhiteSpace($name)) { continue }
    if ($packages.Contains($name)) { Add-Error "Duplicate package registry entry: $name"; continue }
    $packages[$name] = [pscustomobject]@{
        Name = $name
        Path = TStr $block 'path'
        Wave = [int](TInt $block 'wave')
        Assignment = TStr $block 'assignment'
    }
}
if ($packages.Count -ne 45 -or (TInt $packageBlocks[0] 'package_count') -ne 45) {
    Add-Error "Expected 45 package registry entries; parsed $($packages.Count)."
}

# Function registry: three foundation packets plus package-local function packets.
$functionRegistry = [ordered]@{}
$foundationBlocks = [regex]::Split($text.functions, '(?m)^\[\[foundation\]\]\s*$')
for ($i = 1; $i -lt $foundationBlocks.Count; $i++) {
    $block = $foundationBlocks[$i]
    $name = TStr $block 'package'
    if ($functionRegistry.Contains($name)) { Add-Error "Duplicate function registry entry: $name"; continue }
    $functionRegistry[$name] = [pscustomobject]@{
        Assignment = TStr $block 'assignment'
        Primary = TStr $block 'primary_contract'
        WriteScope = TStr $block 'write_scope'
    }
}
$functionBlocks = [regex]::Split($text.functions, '(?m)^\[\[package\]\]\s*$')
for ($i = 1; $i -lt $functionBlocks.Count; $i++) {
    $block = $functionBlocks[$i]
    $name = TStr $block 'name'
    if ($functionRegistry.Contains($name)) { Add-Error "Duplicate function registry entry: $name"; continue }
    $functionRegistry[$name] = [pscustomobject]@{
        Assignment = TStr $block 'assignment'
        Primary = TStr $block 'functions'
        WriteScope = TStr $block 'write_scope'
    }
}
if ($functionRegistry.Count -ne 45 -or -not (Same-Set @($functionRegistry.Keys) @($packages.Keys))) {
    Add-Error 'Function registry package closure differs from the 45-package registry.'
}
foreach ($name in $functionRegistry.Keys) {
    if (-not $packages.Contains($name)) { continue }
    $function = $functionRegistry[$name]
    $package = $packages[$name]
    if ($function.Assignment -cne $package.Assignment) { Add-Error "$name assignment differs across registries." }
    if ($function.WriteScope -cne ($package.Path + '/**')) { Add-Error "$name write scope differs from package path." }
    Validate-File $name ($package.Path + '/AGENTS.md') 'package AGENTS'
    Validate-File $name $function.Assignment 'assignment'
    Validate-File $name $function.Primary 'primary function/contract'
}

# Central gate registry.
$gateBlocks = [regex]::Split($text.gates, '(?m)^\[\[gate\]\]\s*$')
$gateIds = [System.Collections.Generic.HashSet[string]]::new([StringComparer]::Ordinal)
for ($i = 1; $i -lt $gateBlocks.Count; $i++) { [void]$gateIds.Add((TStr $gateBlocks[$i] 'id')) }
if (-not (Same-Set @($gateIds) @('G0','G1','G2','G3','G4','G5','G6'))) {
    Add-Error 'Central gate set must remain exactly G0 through G6.'
}

# Exact stage model.
$expectedPackages = [ordered]@{
    W0 = @('search-contracts','search-domain','search-ports')
    W1 = @('search-config','search-runtime-owner','search-os-secrets','search-control-redb','search-provider-protocol','eliot-searchd','eliot-search')
    W2 = @('search-source-admission','search-source-registry','search-source-identity','search-safe-reader','search-revision-store','search-materializer','search-unitizer','eliot-searchd')
    W3 = @('search-lexical','search-projection-planner','search-point-identity','search-qdrant-supervisor','search-qdrant-bridge','search-publication','search-epoch-pins','search-index-reclaimer','eliot-searchd')
    W4 = @('search-access','search-query-planner','search-retrieval-executor','search-candidate-validator','search-handles','search-result-projector','search-continuation','search-eval','eliot-searchd')
    W5 = @('search-source-reconcile','search-overlay','search-code-enricher','eliot-searchd')
    W6 = @('search-exact','search-subject-resolver','search-comparator','eliot-searchd')
    W7 = @('search-retention','search-revision-store','search-access','search-handles','search-continuation','search-candidate-validator','search-publication','search-index-reclaimer','eliot-searchd')
    W8 = @('search-provider-protocol','eliot-searchd','eliot-search','search-eliot-adapter','search-research-export-adapter')
    W9 = @('search-eval')
    W10 = @('search-model-provider','eliot-search-model-worker','eliot-search-doc-worker','eliot-searchd','search-qdrant-bridge','search-publication','search-epoch-pins','search-index-reclaimer','search-eval')
}
$expectedContribution = [ordered]@{ W0='G0'; W1='G1'; W2='G1'; W3='G2'; W4='G2'; W5='G3'; W6='G3'; W7=''; W8='G4'; W9='G5'; W10='G6' }
$expectedCloses = [ordered]@{ W0=$true; W1=$false; W2=$true; W3=$false; W4=$true; W5=$false; W6=$true; W7=$false; W8=$true; W9=$true; W10=$true }
$expectedReceipt = [ordered]@{ W0='W0'; W1='W1'; W2='W2_G1'; W3='W3'; W4='W4_G2'; W5='W5'; W6='W6_G3'; W7='W7_LIFECYCLE'; W8='W8_G4'; W9='W9_G5'; W10='W10_G6' }
$expectedRequiredGates = [ordered]@{
    W0=@(); W1=@('G0'); W2=@('G0'); W3=@('G1'); W4=@('G1'); W5=@('G2');
    W6=@('G2'); W7=@('G3'); W8=@('G3'); W9=@('G4'); W10=@('G5')
}
$expectedRequiredReceipts = [ordered]@{
    W0=@(); W1=@('W0'); W2=@('W1'); W3=@('W2_G1'); W4=@('W3'); W5=@('W4_G2');
    W6=@('W5'); W7=@('W6_G3'); W8=@('W7_LIFECYCLE'); W9=@('W7_LIFECYCLE','W8_G4');
    W10=@('W9_G5')
}

if ((TStr $text.stages 'status') -cne 'bounded-stage-registry') { Add-Error 'Stage registry status is invalid.' }
if ((TInt $text.stages 'stage_count') -ne 11) { Add-Error 'Stage registry must declare eleven stages.' }
if ((TInt $text.stages 'assignment_count') -ne 68) { Add-Error 'Stage registry must declare 68 stage-package assignments.' }
if ((TStr $text.stages 'current_stage') -cne 'W0' -or (TStr $text.stages 'current_phase') -cne 'P00' -or (TInt $text.stages 'current_wave') -ne 0) {
    Add-Error 'Stage registry current state must remain W0/P00.'
}

$stageBlocks = [regex]::Split($text.stages, '(?m)^\[\[stage\]\]\s*$')
$stages = [ordered]@{}
$totalAssignments = 0
for ($i = 1; $i -lt $stageBlocks.Count; $i++) {
    $block = $stageBlocks[$i]
    $id = TStr $block 'id'
    $wave = [int](TInt $block 'wave')
    if ($stages.Contains($id)) { Add-Error "Duplicate stage: $id"; continue }
    if ($id -cne ('W' + $wave)) { Add-Error "Stage ID/wave mismatch: $id/W$wave" }
    if (-not $expectedPackages.Contains($id)) { Add-Error "Unexpected stage: $id"; continue }

    $status = TStr $block 'status'
    if ($id -eq 'W0') {
        if ($status -cne 'ACTIVE_PACKAGE_ONLY') { Add-Error 'W0 status must remain ACTIVE_PACKAGE_ONLY.' }
    } elseif ($status -cne 'BLOCKED') { Add-Error "$id must remain BLOCKED." }

    $packagesAtStage = @(TArray $block 'packages')
    if (-not (Same-Set $packagesAtStage $expectedPackages[$id])) { Add-Error "$id package set is invalid." }
    $totalAssignments += $packagesAtStage.Count

    $contribution = TStr $block 'contributes_to_gate'
    if ($contribution -cne $expectedContribution[$id]) { Add-Error "$id gate contribution is invalid." }
    if ($contribution -and -not $gateIds.Contains($contribution)) { Add-Error "$id references unknown contribution gate $contribution." }
    if ((TBool $block 'closes_gate') -ne $expectedCloses[$id]) { Add-Error "$id closes_gate is invalid." }
    if ((TStr $block 'completion_receipt') -cne $expectedReceipt[$id]) { Add-Error "$id completion receipt is invalid." }
    if (-not (Same-Set @(TArray $block 'requires_accepted_gates') $expectedRequiredGates[$id])) { Add-Error "$id required gate set is invalid." }
    if (-not (Same-Set @(TArray $block 'requires_accepted_receipts') $expectedRequiredReceipts[$id])) { Add-Error "$id required receipt set is invalid." }

    $implementationPacket = TStr $block 'implementation_packet'
    $machinePacket = TStr $block 'machine_packet'
    $shared = @(TArray $block 'shared_read_set')
    Validate-File $id $implementationPacket 'implementation packet'
    Validate-File $id $machinePacket 'machine packet'
    if ($shared -cnotcontains $implementationPacket) { Add-Error "$id shared read set omits its implementation packet." }
    if ($machinePacket -and $shared -ccontains $machinePacket) { Add-Error "$id exposes integration machine packet in ordinary agent context." }
    if ((Unique-Strings $shared).Count -ne $shared.Count) { Add-Error "$id shared read set contains duplicate files." }
    foreach ($file in $shared) { Validate-File $id $file 'shared read-set' }

    foreach ($packageName in $packagesAtStage) {
        if (-not $packages.Contains($packageName)) { Add-Error "$id references unknown package $packageName."; continue }
        if ($packages[$packageName].Wave -gt $wave) { Add-Error "$id includes later earliest-wave package $packageName." }
    }

    $stages[$id] = [pscustomobject]@{
        Id = $id
        Wave = $wave
        Packages = $packagesAtStage
        Shared = $shared
        ImplementationPacket = $implementationPacket
        MachinePacket = $machinePacket
    }
}
$expectedStageIds = @(0..10 | ForEach-Object { 'W' + $_ })
if (-not (Same-Set @($stages.Keys) $expectedStageIds)) { Add-Error 'Stage IDs must be exactly W0 through W10.' }
if ($totalAssignments -ne 68) { Add-Error "Parsed stage assignment count is $totalAssignments, expected 68." }

# Every package first appears at its registry wave; each later occurrence has an immediate prior stage.
$firstStage = [ordered]@{}
$lastStage = [ordered]@{}
$priorStageByReuse = [ordered]@{}
$reusedKeys = [System.Collections.Generic.List[string]]::new()
foreach ($stage in @($stages.Values | Sort-Object Wave)) {
    foreach ($packageName in $stage.Packages) {
        if (-not $firstStage.Contains($packageName)) {
            $firstStage[$packageName] = $stage.Id
            if ($packages[$packageName].Wave -ne $stage.Wave) {
                Add-Error "$packageName first appears at $($stage.Id), but registry earliest wave is W$($packages[$packageName].Wave)."
            }
        } else {
            $key = "$($stage.Id).$packageName"
            $reusedKeys.Add($key)
            $priorStageByReuse[$key] = $lastStage[$packageName]
        }
        $lastStage[$packageName] = $stage.Id
    }
}
if (-not (Same-Set @($firstStage.Keys) @($packages.Keys))) { Add-Error 'Every package must appear in the stage registry.' }

# Exact later-stage supplement and additional-file sets.
$expectedSupplements = [ordered]@{
    'W2.eliot-searchd' = @()
    'W3.eliot-searchd' = @()
    'W4.eliot-searchd' = @()
    'W5.eliot-searchd' = @()
    'W6.eliot-searchd' = @()
    'W7.eliot-searchd' = @()
    'W7.search-revision-store' = @()
    'W7.search-access' = @('crates/search-query/search-access/W7_HARDENING.md')
    'W7.search-handles' = @('crates/search-query/search-handles/W7_HARDENING.md')
    'W7.search-continuation' = @('crates/search-query/search-continuation/W7_HARDENING.md')
    'W7.search-candidate-validator' = @('crates/search-query/search-candidate-validator/W7_HARDENING.md')
    'W7.search-publication' = @('crates/search-index-qdrant/search-publication/W7_HARDENING.md')
    'W7.search-index-reclaimer' = @('crates/search-index-qdrant/search-index-reclaimer/W7_HARDENING.md')
    'W8.search-provider-protocol' = @('crates/search-provider-protocol/W8_HARDENING.md')
    'W8.eliot-searchd' = @('bins/eliot-searchd/W8_INTEGRATION.md')
    'W8.eliot-search' = @('bins/eliot-search/W8_CLIENT.md')
    'W9.search-eval' = @()
    'W10.eliot-searchd' = @('bins/eliot-searchd/W10_INTEGRATION.md')
    'W10.search-qdrant-bridge' = @('crates/search-index-qdrant/search-qdrant-bridge/P18_SCALE.md')
    'W10.search-publication' = @('crates/search-index-qdrant/search-publication/P18_SCALE.md')
    'W10.search-epoch-pins' = @('crates/search-index-qdrant/search-epoch-pins/P18_SCALE.md')
    'W10.search-index-reclaimer' = @('crates/search-index-qdrant/search-index-reclaimer/P18_SCALE.md')
    'W10.search-eval' = @('crates/search-eval/W10_OPTIONAL_EVALUATION.md')
}
$expectedAdditional = [ordered]@{
    'W7.search-revision-store' = @('config/sections/revision_store.md')
    'W7.search-handles' = @('config/sections/handles.md')
    'W7.search-continuation' = @('config/sections/continuations.md')
    'W7.search-index-reclaimer' = @('config/sections/index_reclaim.md')
    'W8.search-provider-protocol' = @('config/sections/protocol.md')
    'W8.eliot-searchd' = @('config/sections/protocol.md')
    'W10.eliot-searchd' = @('config/sections/optional_profiles.md')
    'W10.search-qdrant-bridge' = @('qualification/optional-depth/scale-profile.toml')
    'W10.search-publication' = @('qualification/optional-depth/scale-profile.toml')
    'W10.search-epoch-pins' = @('qualification/optional-depth/scale-profile.toml')
    'W10.search-index-reclaimer' = @('qualification/optional-depth/scale-profile.toml')
    'W10.search-eval' = @(
      'qualification/optional-depth/baseline.toml',
      'qualification/optional-depth/probes.toml',
      'qualification/optional-depth/gate-map.toml',
      'qualification/optional-depth/fixture-owners.toml'
    )
}

if ((TStr $text.readsets 'status') -cne 'bounded-stage-overrides') { Add-Error 'Stage read-set registry status is invalid.' }
$declaredOverrideCount = [int](TInt $text.readsets 'override_count')
$staticCeiling = [int](TInt $text.readsets 'max_static_context_files')
if ($declaredOverrideCount -ne 23 -or $staticCeiling -ne 16) { Add-Error 'Stage read-set count/ceiling must remain 23/16.' }
if ((TInt $text.readsets 'max_ticket_handoff_receipts') -ne 16 -or (TInt $text.readsets 'max_ticket_fixture_refs') -ne 16) {
    Add-Error 'Ticket handoff/fixture ceilings must remain 16/16.'
}
if ((TStr $text.readsets 'ordinary_agent_architecture_access') -cne 'exception-only') { Add-Error 'Architecture access must remain exception-only.' }
if (TBool $text.readsets 'dependency_implementation_reads_allowed') { Add-Error 'Dependency implementation reads must remain forbidden.' }
if (TBool $text.readsets 'previous_stage_document_replay_allowed') { Add-Error 'Previous-stage document replay must remain forbidden.' }

$overrideBlocks = [regex]::Split($text.readsets, '(?m)^\[\[override\]\]\s*$')
$overrides = [ordered]@{}
$maxObservedContext = 0
$baseStaticFileCount = 6 # root/package AGENTS, authority, protocol, assignment, primary contract
for ($i = 1; $i -lt $overrideBlocks.Count; $i++) {
    $block = $overrideBlocks[$i]
    $id = TStr $block 'id'
    $stageId = TStr $block 'stage'
    $packageName = TStr $block 'package'
    $wave = [int](TInt $block 'wave')
    if ($overrides.Contains($id)) { Add-Error "Duplicate stage override: $id"; continue }
    if ($id -cne "$stageId.$packageName") { Add-Error "Override ID mismatch: $id" }
    if (-not $stages.Contains($stageId) -or -not $packages.Contains($packageName)) { Add-Error "Override $id references unknown stage/package."; continue }
    if ($stageId -eq $firstStage[$packageName]) { Add-Error "Earliest-wave package $id must not have a stage override." }
    if ($wave -ne $stages[$stageId].Wave) { Add-Error "Override $id wave mismatch." }
    if ((TStr $block 'status') -cne 'BLOCKED') { Add-Error "Override $id must remain BLOCKED." }
    if ((TStr $block 'base_stage') -cne $firstStage[$packageName]) { Add-Error "Override $id base_stage is invalid." }
    if ((TStr $block 'prior_stage') -cne $priorStageByReuse[$id]) { Add-Error "Override $id prior_stage is invalid." }
    if (-not (TBool $block 'replace_previous_stage_context')) { Add-Error "Override $id must replace previous-stage context." }
    if (-not (TBool $block 'accepted_prior_stage_handoff_only')) { Add-Error "Override $id must use accepted prior-stage handoffs only." }
    if ((TStr $block 'architecture_access') -cne 'exception-only') { Add-Error "Override $id architecture access is invalid." }
    if (TBool $block 'dependency_implementation_reads_allowed') { Add-Error "Override $id permits dependency implementation reads." }
    if (TBool $block 'shared_registry_edits_allowed') { Add-Error "Override $id permits shared registry edits." }

    $supplements = @(TArray $block 'supplements')
    $additional = @(TArray $block 'additional_files')
    $forbidden = @(TArray $block 'forbidden_prior_stage_packets')
    $handoffs = @(TArray $block 'required_prior_handoffs')
    if (-not $expectedSupplements.Contains($id) -or -not (Same-Set $supplements $expectedSupplements[$id])) {
        Add-Error "Override $id supplement set is invalid."
    }
    $expectedAdditionalForId = if ($expectedAdditional.Contains($id)) { $expectedAdditional[$id] } else { @() }
    if (-not (Same-Set $additional $expectedAdditionalForId)) { Add-Error "Override $id additional-file set is invalid." }
    if ($forbidden.Count -eq 0) { Add-Error "Override $id has no forbidden prior-stage packet." }
    if ($handoffs.Count -eq 0) { Add-Error "Override $id has no required prior handoff." }

    $basePacket = $stages[$firstStage[$packageName]].ImplementationPacket
    $priorPacket = $stages[$priorStageByReuse[$id]].ImplementationPacket
    if ($forbidden -cnotcontains $basePacket) { Add-Error "Override $id does not forbid its base-stage implementation packet." }
    if ($forbidden -cnotcontains $priorPacket) { Add-Error "Override $id does not forbid its immediate prior-stage implementation packet." }
    foreach ($file in @($supplements + $additional + $forbidden)) { Validate-File $id $file 'override' }
    foreach ($file in $forbidden) {
        if ($stages[$stageId].Shared -contains $file -or $supplements -contains $file -or $additional -contains $file) {
            Add-Error "Override $id includes forbidden prior-stage packet $file in its active context."
        }
    }

    $expectedScope = $functionRegistry[$packageName].WriteScope
    if ((TStr $block 'write_scope') -cne $expectedScope) { Add-Error "Override $id write scope differs from function registry." }
    $activeStageFiles = Unique-Strings @($stages[$stageId].Shared + $supplements + $additional)
    $computedCount = $baseStaticFileCount + $activeStageFiles.Count
    $declaredCount = [int](TInt $block 'static_context_file_count')
    if ($computedCount -ne $declaredCount) { Add-Error "Override $id static context count $declaredCount != computed $computedCount." }
    if ($declaredCount -gt $staticCeiling) { Add-Error "Override $id exceeds static context ceiling $staticCeiling." }
    if ($declaredCount -gt $maxObservedContext) { $maxObservedContext = $declaredCount }

    $overrides[$id] = [pscustomobject]@{ Stage = $stageId; Package = $packageName; Count = $declaredCount }
}
if ($overrides.Count -ne $declaredOverrideCount) { Add-Error "Parsed $($overrides.Count) overrides; declared $declaredOverrideCount." }
if (-not (Same-Set @($overrides.Keys) $reusedKeys.ToArray())) {
    Add-Error 'Later-stage reused package set differs from stage override registry.'
}

# Narrow package-stage deltas are structurally complete.
Require-Tokens $paths.w8_client $text.w8_client @(
    'parse_client_invocation', 'resolve_local_endpoint', 'pair_and_bind', 'open_session',
    'fetch_capabilities', 'build_recipe_request', 'execute_request', 'render_terminal',
    'expand_handle', 'classify_exit_status', 'close_session', 'Typed failures',
    'Required tests / qualification evidence'
)
Require-Tokens $paths.w10_eval $text.w10_eval @(
    'validate_optional_campaign', 'freeze_candidate_comparison', 'build_optional_trial_schedule',
    'score_incremental_quality', 'score_incremental_cost', 'audit_optional_noninterference',
    'validate_optional_fault_matrix', 'validate_removal_and_p15_regression',
    'compare_optional_candidate', 'build_g6_evidence_candidate', 'verify_g6_independent_review',
    'Typed failures', 'Required tests / qualification evidence'
)

# Launch state remains unchanged but binds all machine context registries.
foreach ($entry in [ordered]@{
    package_registry_path = 'swarm/crates.toml'
    function_packet_registry_path = 'swarm/function-packets.toml'
    stage_registry_path = 'swarm/stages.toml'
    stage_readset_registry_path = 'swarm/stage-readsets.toml'
}.GetEnumerator()) {
    if ((TStr $text.launch $entry.Key) -cne $entry.Value) { Add-Error "Launch state $($entry.Key) is inconsistent." }
}
if ((TStr $text.launch 'active_stage') -cne 'P00' -or (TInt $text.launch 'active_wave') -ne 0) { Add-Error 'Launch must remain P00/W0.' }
$authorized = @(TArray $text.launch 'authorized_packages')
$conditional = @(TArray $text.launch 'conditional_packages')
if (-not (Same-Set $authorized @('search-contracts'))) { Add-Error 'Only search-contracts may be authorized.' }
if (-not (Same-Set $conditional @('search-domain','search-ports'))) { Add-Error 'Conditional W0 package set is invalid.' }

# Human authority and launch procedure must expose replacement semantics.
Require-Tokens $paths.agents $text.agents @('swarm/stages.toml','swarm/stage-readsets.toml','replace previous-stage documents')
Require-Tokens $paths.authority $text.authority @('swarm/stages.toml','swarm/stage-readsets.toml','accepted prior-stage handoff')
Require-Tokens $paths.assignment_protocol $text.assignment_protocol @('stage-specific read set','swarm/stage-readsets.toml','prior-stage implementation packet')
Require-Tokens $paths.handoff $text.handoff @('twenty-three later-stage assignments','static package context is capped at sixteen files','W7_LIFECYCLE')
Require-Tokens $paths.swarm_readme $text.swarm_readme @('45 one-writer packages','stages.toml','stage-readsets.toml')
Require-Tokens $paths.tools_readme $text.tools_readme @('validate-stage-readsets.ps1','23 later-stage overrides','68 stage-package assignments')

# All workflows remain manual-only and read-only.
$workflowFiles = @(Get-ChildItem (Join-Path $Root '.github/workflows') -Filter '*.yml' -File)
foreach ($file in $workflowFiles) {
    $workflowText = [IO.File]::ReadAllText($file.FullName)
    if ($workflowText -match '(?m)^\s*(pull_request|pull_request_target|push|schedule|workflow_run|repository_dispatch|workflow_call|merge_group):') {
        Add-Error "Automatic workflow trigger found in $($file.Name)."
    }
    if ($workflowText.IndexOf('workflow_dispatch:', [StringComparison]::Ordinal) -lt 0) {
        Add-Error "Workflow $($file.Name) lacks workflow_dispatch."
    }
}
Require-Tokens $paths.workflow $text.workflow @('contents: read','persist-credentials: false','validate-stage-readsets.ps1')

$result = [ordered]@{
    ok = ($errors.Count -eq 0)
    packages = $packages.Count
    function_packets = $functionRegistry.Count
    stages = $stages.Count
    stage_assignments = $totalAssignments
    reused_package_assignments = $reusedKeys.Count
    stage_overrides = $overrides.Count
    maximum_static_context_files = $maxObservedContext
    workflows = $workflowFiles.Count
    launch_stage = TStr $text.launch 'active_stage'
    launch_wave = TInt $text.launch 'active_wave'
    authorized = $authorized
    conditional = $conditional
    warnings = @($warnings)
    errors = @($errors)
}

if ($Json) { $result | ConvertTo-Json -Depth 8 }
else {
    Write-Host 'ELIOT Search stage/read-set validation'
    Write-Host "packages=$($result.packages) stages=$($result.stages) assignments=$($result.stage_assignments) overrides=$($result.stage_overrides) max_context=$($result.maximum_static_context_files)"
    foreach ($warning in $warnings) { Write-Warning $warning }
    foreach ($error in $errors) { Write-Host "ERROR: $error" -ForegroundColor Red }
    if ($result.ok) { Write-Host 'PASS' -ForegroundColor Green }
}
if (-not $result.ok) { exit 1 }
