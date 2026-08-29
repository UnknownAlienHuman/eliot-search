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
    $match = [regex]::Match($Text, ('(?m)^{0}\s*=\s*(-?\d+)\s*$' -f [regex]::Escape($Key)))
    if (-not $match.Success) {
        if ($Required) { Add-Error "Missing TOML integer '$Key'." }
        return 0
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
function TRaw([string]$Text, [string]$Key, [bool]$Required = $true) {
    $match = [regex]::Match($Text, ('(?m)^{0}\s*=\s*(.+?)\s*$' -f [regex]::Escape($Key)))
    if (-not $match.Success) {
        if ($Required) { Add-Error "Missing TOML value '$Key'." }
        return ''
    }
    $match.Groups[1].Value.Trim()
}
function TArray([string]$Text, [string]$Key) {
    $match = [regex]::Match($Text, ('(?ms)^{0}\s*=\s*\[(.*?)\]' -f [regex]::Escape($Key)))
    if (-not $match.Success) { return @() }
    @([regex]::Matches($match.Groups[1].Value, '"([^"\r\n]+)"') | ForEach-Object { $_.Groups[1].Value })
}
function Section([string]$Text, [string]$Name) {
    $match = [regex]::Match($Text, ('(?ms)^\[{0}\]\s*(.*?)(?=^\[|\z)' -f [regex]::Escape($Name)))
    if (-not $match.Success) { Add-Error "Missing TOML section [$Name]."; return '' }
    $match.Groups[1].Value
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
function Require-Tokens([string]$Path, [string]$Text, [string[]]$Tokens) {
    foreach ($token in $Tokens) {
        if (-not $Text.Contains($token, [StringComparison]::Ordinal)) {
            Add-Error "${Path} is missing required token: $token"
        }
    }
}
function Parse-Fields([string]$Text) {
    $fields = [ordered]@{}
    $pattern = '(?ms)^\[\[([A-Za-z0-9_]+)\.field\]\]\s*(.*?)(?=^\[\[[A-Za-z0-9_]+\.field\]\]|^\[[A-Za-z0-9_]+\]\s*$|\z)'
    foreach ($match in [regex]::Matches($Text, $pattern)) {
        $section = $match.Groups[1].Value
        $block = $match.Groups[2].Value
        $name = TStr $block 'name'
        $key = "$section.$name"
        if ($fields.Contains($key)) { Add-Error "Duplicate optional-depth setting '$key'."; continue }
        $fields[$key] = [pscustomobject]@{
            Mode = TStr $block 'mode'
            Default = TRaw $block 'default'
            Min = TRaw $block 'min' $false
            Max = TRaw $block 'max' $false
            Action = TStr $block 'change_action' $false
        }
    }
    $fields
}

$paths = [ordered]@{
    manifest = 'docs/optional/manifest.toml'
    contract = 'docs/optional/W10_OPTIONAL_DEPTH_CONTRACTS_1.0.md'
    handoff = 'docs/handoff/W10_IMPLEMENTATION_PACKET.md'
    model_functions = 'crates/search-model-provider/FUNCTIONS.md'
    model_worker = 'bins/eliot-search-model-worker/FUNCTIONS.md'
    doc_worker = 'bins/eliot-search-doc-worker/FUNCTIONS.md'
    daemon = 'bins/eliot-searchd/W10_INTEGRATION.md'
    bridge_scale = 'crates/search-index-qdrant/search-qdrant-bridge/P18_SCALE.md'
    publication_scale = 'crates/search-index-qdrant/search-publication/P18_SCALE.md'
    pins_scale = 'crates/search-index-qdrant/search-epoch-pins/P18_SCALE.md'
    reclaimer_scale = 'crates/search-index-qdrant/search-index-reclaimer/P18_SCALE.md'
    qualification = 'qualification/optional-depth/W10_QUALIFICATION.md'
    baseline = 'qualification/optional-depth/baseline.toml'
    model_profile = 'qualification/optional-depth/model-profile.toml'
    document_profile = 'qualification/optional-depth/document-profile.toml'
    scale_profile = 'qualification/optional-depth/scale-profile.toml'
    probes = 'qualification/optional-depth/probes.toml'
    gate_map = 'qualification/optional-depth/gate-map.toml'
    fixtures = 'qualification/optional-depth/fixture-owners.toml'
    settings = 'config/w10-optional-depth.toml'
    settings_doc = 'docs/config/W10_OPTIONAL_DEPTH_SETTINGS_1.0.md'
    optional_section = 'config/sections/optional_profiles.md'
    example = 'config/eliot-search.example.toml'
    swarm = 'swarm/w10-optional-depth.toml'
    gates = 'swarm/gates.toml'
    launch = 'swarm/launch-state.toml'
    workflow = '.github/workflows/w10-optional-depth.yml'
}
$text = [ordered]@{}
foreach ($entry in $paths.GetEnumerator()) { $text[$entry.Key] = Read-Required $entry.Value }

if ((TStr $text.manifest 'status') -cne 'contract-only') { Add-Error 'W10 manifest must remain contract-only.' }
foreach ($flag in @('implementation_authorized', 'optional_depth_authorized', 'provider_selected')) {
    if (TBool $text.manifest $flag) { Add-Error "W10 manifest cannot set $flag=true." }
}
$ownerBlocks = [regex]::Split($text.manifest, '(?m)^\[\[owner\]\]\s*$')
$ownerPackages = [System.Collections.Generic.List[string]]::new()
$ownerRoles = [System.Collections.Generic.List[string]]::new()
for ($i = 1; $i -lt $ownerBlocks.Count; $i++) {
    $block = $ownerBlocks[$i]
    $role = TStr $block 'role'
    $package = TStr $block 'package'
    $ownerRoles.Add($role)
    $ownerPackages.Add($package)
    $function = TStr $block 'functions' $false
    $contract = TStr $block 'contract' $false
    $relative = if ($function) { $function } else { $contract }
    if (-not $relative -or -not (Test-Path (Join-Path $Root $relative) -PathType Leaf)) { Add-Error "W10 owner $package lacks a valid packet path." }
}
$expectedOwnerPackages = @('search-model-provider', 'eliot-search-model-worker', 'eliot-search-doc-worker', 'eliot-searchd', 'search-qdrant-bridge', 'search-publication', 'search-epoch-pins', 'search-index-reclaimer')
if (-not (Same-Set @($ownerPackages) $expectedOwnerPackages)) { Add-Error 'W10 owner package set is invalid.' }
if (@($ownerRoles | Where-Object { $_ -ceq 'package' }).Count -ne 3 -or @($ownerRoles | Where-Object { $_ -ceq 'integration' }).Count -ne 1 -or @($ownerRoles | Where-Object { $_ -ceq 'scale-package' }).Count -ne 4) { Add-Error 'W10 owner role counts must be 3 package, 1 integration, 4 scale-package.' }

Require-Tokens $paths.contract $text.contract @(
    '## 2. Gate chain', '## 4. Model profile identity', '## 5. Model semantics',
    '## 6. Model worker boundary', '## 8. Document provider identity', '## 9. Document worker no-execute boundary',
    '## 11. Measured material benefit', '## 12. Removal and baseline restoration',
    '## 13. Advanced scale trigger', '## 14. P18 migration state machine',
    '## 18. Hard stop conditions', 'G6: NOT ACCEPTED'
)
Require-Tokens $paths.model_functions $text.model_functions @(
    '### `validate_profile_descriptor', '### `encode_documents', '### `encode_query',
    '### `rerank', '### `validate_rerank_output', '### `classify_profile_capability',
    '### `validate_incremental_benefit_receipt', '### `prepare_removal',
    '## Cancellation, deadlines and retry', '## Typed failures', '## Required tests / qualification evidence'
)
Require-Tokens $paths.model_worker $text.model_worker @(
    '### `validate_startup', '### `verify_inherited_containment', '### `load_qualified_provider',
    '### `open_private_session', '### `serve_encode', '### `serve_rerank', '### `cancel_request',
    '### `shutdown_and_remove', '## Crash and retry semantics', '## Typed failures', '## Required tests / qualification evidence'
)
Require-Tokens $paths.doc_worker $text.doc_worker @(
    '### `validate_provider_profile', '### `verify_inherited_sandbox', '### `inspect_container_and_input',
    '### `materialize', '### `validate_materialization_output', '### `cleanup_request_workspace',
    '### `shutdown_and_remove', '## Crash and retry semantics', '## Typed failures', '## Required tests / qualification evidence'
)
Require-Tokens $paths.daemon $text.daemon @(
    '### `evaluate_optional_candidate', '### `plan_optional_activation', '### `commit_optional_activation',
    '### `publish_optional_capability_snapshot', '### `plan_optional_removal', '### `commit_baseline_restore',
    '### `drain_and_remove_optional', '### `plan_scale_candidate', '### `commit_scale_route_switch',
    '### `rollback_scale_route', '## Typed failures', '## Required tests / evidence'
)
Require-Tokens $paths.bridge_scale $text.bridge_scale @('probe_scale_capabilities', 'create_scale_candidate_collection', 'validate_scale_query_equivalence', 'active collection schema/topology is never mutated in place')
Require-Tokens $paths.publication_scale $text.publication_scale @('BASE_BUILT_AT_R0', 'FINAL_BARRIER_ENTERED', 'ROUTE_SWITCH_COMMITTED', 'rollback_scale_intent')
Require-Tokens $paths.pins_scale $text.pins_scale @('fence_old_route_for_new_pins', 'snapshot_route_drain', 'unknown/stale state fails closed')
Require-Tokens $paths.reclaimer_scale $text.reclaimer_scale @('validate_retired_route_manifest', 'plan_old_route_reclaim', 'ordinary route reclaim cannot satisfy purge')
Require-Tokens $paths.handoff $text.handoff @('one candidate class/profile', 'Required G6 evidence', 'Hard stop conditions', '45 DISABLED')

if ((TStr $text.swarm 'status') -cne 'BLOCKED' -or (TStr $text.swarm 'requires_accepted_gate') -cne 'G5') { Add-Error 'W10 swarm packet must remain BLOCKED behind G5.' }
foreach ($flag in @('implementation_authorized', 'optional_depth_authorized')) { if (TBool $text.swarm $flag) { Add-Error "W10 swarm packet cannot set $flag=true." } }
if (-not (TBool $text.swarm 'one_candidate_per_ticket')) { Add-Error 'W10 must require one candidate per ticket.' }
$packetBlocks = [regex]::Split($text.swarm, '(?m)^\[\[packet\]\]\s*$')
$packetPackages = [System.Collections.Generic.List[string]]::new()
for ($i = 1; $i -lt $packetBlocks.Count; $i++) {
    $block = $packetBlocks[$i]
    $package = TStr $block 'package'
    if ($packetPackages.Contains($package)) { Add-Error "Duplicate W10 packet package: $package" }
    $packetPackages.Add($package)
    $function = TStr $block 'functions' $false
    $contract = TStr $block 'contract' $false
    $relative = if ($function) { $function } else { $contract }
    if (-not $relative -or -not (Test-Path (Join-Path $Root $relative) -PathType Leaf)) { Add-Error "W10 packet $package lacks a valid function/contract file." }
    $writeScope = TStr $block 'write_scope'
    if (-not $writeScope.StartsWith(($package -eq 'eliot-searchd' -or $package -like 'eliot-search-*' ? 'bins/' : 'crates/'), [StringComparison]::Ordinal)) { Add-Warning "Review W10 write scope for $package: $writeScope" }
    foreach ($readPath in (TArray $block 'read_set')) {
        if (-not (Test-Path (Join-Path $Root $readPath) -PathType Leaf)) { Add-Error "W10 packet $package references missing read-set file: $readPath" }
    }
}
if (-not (Same-Set @($packetPackages) $expectedOwnerPackages)) { Add-Error 'W10 swarm packet package set differs from owner manifest.' }
$expectedEvidence = @('dedicated_optional_profile_adr', 'exact_provider_artifact_qualification', 'measured_material_benefit', 'removal_or_uninstall_fallback', 'migration_and_rollback_when_applicable')
if (-not (Same-Set @(TArray $text.swarm 'required_evidence_ids') $expectedEvidence)) { Add-Error 'W10 required evidence set is invalid.' }

if ((TInt $text.launch 'active_wave') -ne 0 -or (TStr $text.launch 'active_stage') -cne 'P00') { Add-Error 'Launch authority must remain P00/W0.' }
if (-not (Same-Set @(TArray $text.launch 'authorized_packages') @('search-contracts'))) { Add-Error 'Only search-contracts may remain authorized.' }
$optionalDepth = Section $text.launch 'optional_depth'
if ((TStr $optionalDepth 'model_and_document_packages') -cne 'blocked' -or (TStr $optionalDepth 'advanced_scale_packages') -cne 'blocked') { Add-Error 'Optional packages must remain blocked.' }
if ((TStr $optionalDepth 'packet') -cne $paths.swarm -or (TStr $optionalDepth 'qualification') -cne $paths.qualification -or (TStr $optionalDepth 'settings') -cne $paths.settings) { Add-Error 'Launch optional-depth packet paths are inconsistent.' }
if ((TStr $optionalDepth 'accepted_p15_receipt_ref') -ne '' -or (TStr $optionalDepth 'selected_candidate') -cne 'NONE' -or (TArray $optionalDepth 'accepted_candidate_receipts').Count -ne 0) { Add-Error 'Launch state prematurely selects/accepts optional depth.' }
if ((TStr $optionalDepth 'requires_accepted_gate') -cne 'G5' -or (TBool $optionalDepth 'configuration_alone_authorizes')) { Add-Error 'Optional-depth launch gate semantics are invalid.' }

$g6Match = [regex]::Match($text.gates, '(?ms)^\[\[gate\]\]\s*id\s*=\s*"G6"(.*?)(?=^\[\[gate\]\]|\z)')
if (-not $g6Match.Success) { Add-Error 'Central G6 gate block is missing.' }
elseif (-not (Same-Set @(TArray $g6Match.Groups[1].Value 'required_evidence') $expectedEvidence)) { Add-Error 'Central G6 evidence set differs from W10.' }

if ((TStr $text.baseline 'status') -cne 'DISABLED_NOT_SELECTED') { Add-Error 'Optional-depth baseline must remain DISABLED_NOT_SELECTED.' }
foreach ($flag in @('implementation_authorized', 'optional_depth_authorized')) { if (TBool $text.baseline $flag) { Add-Error "Optional-depth baseline cannot set $flag=true." } }
$acceptedBaseline = Section $text.baseline 'accepted_baseline'
$candidate = Section $text.baseline 'candidate'
$activation = Section $text.baseline 'activation'
$common = Section $text.baseline 'common_policy'
$selection = Section $text.baseline 'selection'
if ((TStr $acceptedBaseline 'p15_report_ref') -cne 'UNSELECTED') { Add-Error 'Accepted P15 ref must remain UNSELECTED.' }
if ((TStr $candidate 'profile_class') -cne 'NONE' -or (TStr $candidate 'profile_ref') -cne 'UNSELECTED' -or (TStr $candidate 'status') -cne 'DISABLED') { Add-Error 'No optional candidate may be selected.' }
foreach ($flag in @('compiled_feature_present', 'explicit_configuration_enabled', 'binding_authorized', 'worker_qualified', 'candidate_route_validated', 'gate_receipt_accepted', 'active')) { if (TBool $activation $flag) { Add-Error "Activation flag prematurely enabled: $flag" } }
foreach ($flag in @('network_allowed', 'automatic_download_allowed', 'automatic_upgrade_allowed', 'training_or_learning_allowed', 'persistent_content_cache_allowed', 'unsaved_persistence_allowed', 'generative_answer_authority_allowed', 'client_admission_authority_allowed', 'provider_output_is_source_evidence', 'in_place_schema_change_allowed', 'qdrant_alias_is_commit', 'self_review_allowed', 'self_acceptance_allowed')) { if (TBool $common $flag) { Add-Error "Unsafe optional common policy enabled: $flag" } }
foreach ($flag in @('latest_allowed', 'version_range_allowed', 'floating_revision_allowed', 'documentation_only_acceptance_allowed', 'unit_tests_only_acceptance_allowed', 'compilation_only_acceptance_allowed', 'configuration_only_activation_allowed')) { if (TBool $selection $flag) { Add-Error "Unsafe optional selection flag enabled: $flag" } }

if ((TStr $text.model_profile 'status') -cne 'UNSELECTED' -or TBool $text.model_profile 'enabled') { Add-Error 'Model profile must remain UNSELECTED/disabled.' }
foreach ($key in @('accepted_p15_receipt_ref', 'adr_ref')) { if ((TStr $text.model_profile $key) -cne 'UNSELECTED') { Add-Error "Model $key must remain UNSELECTED." } }
$modelCaps = Section $text.model_profile 'capabilities'
foreach ($flag in @('rerank_only', 'dense_vector', 'multivector', 'generative_answers', 'training_or_learning', 'network', 'automatic_download', 'automatic_upgrade', 'persistent_input_cache')) { if (TBool $modelCaps $flag) { Add-Error "Model capability prematurely enabled: $flag" } }
$modelArtifact = Section $text.model_profile 'artifact'
foreach ($key in @('provider_name', 'provider_source_ref', 'provider_version', 'model_name', 'model_source_ref', 'runtime_backend', 'runtime_version', 'Windows_package_ref')) { if ((TStr $modelArtifact $key) -cne 'UNSELECTED') { Add-Error "Model artifact field $key must remain UNSELECTED." } }

if ((TStr $text.document_profile 'status') -cne 'UNSELECTED' -or TBool $text.document_profile 'enabled') { Add-Error 'Document profile must remain UNSELECTED/disabled.' }
foreach ($key in @('accepted_p15_receipt_ref', 'adr_ref')) { if ((TStr $text.document_profile $key) -cne 'UNSELECTED') { Add-Error "Document $key must remain UNSELECTED." } }
$docArtifact = Section $text.document_profile 'artifact'
foreach ($key in @('provider_name', 'provider_source_ref', 'provider_version', 'runtime_name', 'runtime_version', 'Windows_package_ref')) { if ((TStr $docArtifact $key) -cne 'UNSELECTED') { Add-Error "Document artifact field $key must remain UNSELECTED." } }
if (TBool $docArtifact 'Python_or_Node_runtime') { Add-Error 'Document Python/Node runtime cannot be selected in scaffold.' }
$docSecurity = Section $text.document_profile 'security'
foreach ($flag in @('network_allowed', 'scripts_allowed', 'javascript_allowed', 'macros_allowed', 'ole_actions_allowed', 'hooks_or_filters_allowed', 'shell_or_child_process_allowed', 'remote_resources_allowed', 'credential_prompts_allowed', 'path_traversal_allowed', 'symlink_hardlink_reparse_escape_allowed', 'automatic_download', 'automatic_upgrade')) { if (TBool $docSecurity $flag) { Add-Error "Unsafe document security flag enabled: $flag" } }

if ((TStr $text.scale_profile 'status') -cne 'UNSELECTED' -or TBool $text.scale_profile 'enabled') { Add-Error 'Scale profile must remain UNSELECTED/disabled.' }
foreach ($key in @('accepted_p15_receipt_ref', 'measured_bottleneck_report_ref', 'adr_ref')) { if ((TStr $text.scale_profile $key) -cne 'UNSELECTED') { Add-Error "Scale $key must remain UNSELECTED." } }
$scaleArtifact = Section $text.scale_profile 'artifact'
foreach ($key in @('qdrant_server_version', 'qdrant_client_version', 'Windows_package_ref')) { if ((TStr $scaleArtifact $key) -cne 'UNSELECTED') { Add-Error "Scale artifact field $key must remain UNSELECTED." } }
$topology = Section $text.scale_profile 'topology'
if ((TStr $topology 'profile_name') -cne 'UNSELECTED' -or -not (TBool $topology 'strict_mode_required') -or (TBool $topology 'in_place_change_allowed') -or (TBool $topology 'qdrant_alias_is_commit')) { Add-Error 'Scale topology defaults are unsafe.' }
$migration = Section $text.scale_profile 'migration'
foreach ($flag in @('base_at_r0_required', 'ordered_catch_up_required', 'final_barrier_at_r1_required', 'guarded_redb_route_switch_required', 'old_route_pins_required', 'failed_candidate_discard_required', 'post_switch_rollback_required')) { if (-not (TBool $migration $flag)) { Add-Error "Scale migration requirement disabled: $flag" } }

if ((TStr $text.probes 'status') -cne 'TEMPLATES_DISABLED' -or (TStr $text.probes 'selected_candidate') -cne 'NONE') { Add-Error 'W10 probe templates must remain disabled/unselected.' }
foreach ($flag in @('raw_output_required_for_pass', 'independent_review_required_for_pass')) { if (-not (TBool $text.probes $flag)) { Add-Error "Required W10 probe flag disabled: $flag" } }
$probeDefaults = Section $text.probes 'probe_defaults'
if (-not (TBool $probeDefaults 'mandatory_when_selected') -or (TStr $probeDefaults 'result') -cne 'DISABLED') { Add-Error 'W10 probe defaults must be mandatory-when-selected/DISABLED.' }
foreach ($key in @('raw_output_ref', 'raw_output_digest', 'reviewer_receipt_ref')) { if ((TStr $probeDefaults $key) -ne '') { Add-Error "W10 probe default $key must be empty." } }
$probeIds = [System.Collections.Generic.HashSet[string]]::new([StringComparer]::Ordinal)
$profileProbeIds = [ordered]@{}
$profileEvidenceCounts = [ordered]@{}
foreach ($profile in @('model', 'document', 'scale')) {
    $profileProbeIds[$profile] = [System.Collections.Generic.List[string]]::new()
    $profileEvidenceCounts[$profile] = [ordered]@{}
    foreach ($evidence in $expectedEvidence) { $profileEvidenceCounts[$profile][$evidence] = 0 }
}
$probeBlocks = [regex]::Split($text.probes, '(?m)^\[\[probe\]\]\s*$')
for ($i = 1; $i -lt $probeBlocks.Count; $i++) {
    $block = $probeBlocks[$i]
    $id = TStr $block 'id'
    if (-not $probeIds.Add($id)) { Add-Error "Duplicate W10 probe ID: $id" }
    $profile = TStr $block 'profile'
    $evidence = TStr $block 'evidence_id'
    if (-not $profileProbeIds.Contains($profile)) { Add-Error "Probe $id has unknown profile $profile." }
    else {
        $profileProbeIds[$profile].Add($id)
        if (-not $profileEvidenceCounts[$profile].Contains($evidence)) { Add-Error "Probe $id has unknown G6 evidence $evidence." }
        else { $profileEvidenceCounts[$profile][$evidence]++ }
    }
    [void](TStr $block 'producer')
    [void](TStr $block 'purpose')
    $result = TStr $block 'result' $false
    if ($result -and $result -cne 'DISABLED') { Add-Error "Probe $id has premature result $result." }
    foreach ($key in @('raw_output_ref', 'raw_output_digest', 'reviewer_receipt_ref')) { if (TStr $block $key $false) { Add-Error "Probe $id contains premature $key." } }
}
if ($probeIds.Count -ne 45) { Add-Error "Expected 45 W10 probes; parsed $($probeIds.Count)." }
foreach ($profile in @('model', 'document', 'scale')) {
    if ($profileProbeIds[$profile].Count -ne 15) { Add-Error "Profile $profile must have 15 probes." }
    foreach ($evidence in $expectedEvidence) { if ($profileEvidenceCounts[$profile][$evidence] -ne 3) { Add-Error "Profile $profile evidence $evidence must have 3 probes." } }
}

if ((TStr $text.gate_map 'status') -cne 'CANDIDATE_TEMPLATES_DISABLED' -or (TStr $text.gate_map 'selected_candidate') -cne 'NONE') { Add-Error 'W10 gate map must remain disabled/unselected.' }
$candidateBlocks = [regex]::Split($text.gate_map, '(?m)^\[\[candidate\]\]\s*$')
$candidateProfiles = [System.Collections.Generic.List[string]]::new()
for ($i = 1; $i -lt $candidateBlocks.Count; $i++) {
    $block = $candidateBlocks[$i]
    $profile = TStr $block 'profile'
    $candidateProfiles.Add($profile)
    if ((TStr $block 'status') -cne 'DISABLED') { Add-Error "Gate-map profile $profile must remain DISABLED." }
    $mapped = [System.Collections.Generic.List[string]]::new()
    foreach ($evidence in $expectedEvidence) {
        $ids = @(TArray $block $evidence)
        if ($ids.Count -ne 3) { Add-Error "Gate-map profile $profile evidence $evidence must list 3 probes." }
        foreach ($id in $ids) {
            $mapped.Add($id)
            if (-not $probeIds.Contains($id)) { Add-Error "Gate map references unknown probe $id." }
        }
    }
    if ($profileProbeIds.Contains($profile) -and -not (Same-Set @($mapped) @($profileProbeIds[$profile]))) { Add-Error "Gate-map profile $profile does not cover exactly its probes." }
}
if (-not (Same-Set @($candidateProfiles) @('model', 'document', 'scale'))) { Add-Error 'W10 gate-map candidate set is invalid.' }

if ((TStr $text.settings 'status') -cne 'schema-only' -or (TBool $text.settings 'implementation_authorized') -or (TBool $text.settings 'optional_depth_authorized')) { Add-Error 'W10 settings must remain schema-only/non-authorizing.' }
$fields = Parse-Fields $text.settings
$qualifiedRefs = @(
    'gate.accepted_p15_receipt_ref', 'gate.candidate_adr_ref', 'gate.candidate_qualification_ref',
    'gate.candidate_benefit_ref', 'gate.candidate_removal_ref', 'gate.candidate_migration_rollback_ref',
    'model.profile_ref', 'document.profile_ref', 'scale.profile_ref', 'scale.measured_bottleneck_ref'
)
foreach ($key in $qualifiedRefs) { if (-not $fields.Contains($key) -or $fields[$key].Mode -cne 'QUALIFIED_REF' -or $fields[$key].Default -cne '"UNSELECTED"') { Add-Error "$key must be an UNSELECTED QUALIFIED_REF." } }
$lockedExpected = [ordered]@{
    'gate.compiled_feature_required' = 'true'; 'gate.explicit_configuration_required' = 'true';
    'gate.binding_authorization_required' = 'true'; 'gate.configuration_alone_authorizes' = 'false';
    'gate.one_candidate_per_ticket' = 'true'; 'model.network_allowed' = 'false';
    'model.automatic_download_allowed' = 'false'; 'model.automatic_upgrade_allowed' = 'false';
    'model.training_or_learning_allowed' = 'false'; 'model.generative_answer_allowed' = 'false';
    'model.persistent_input_cache_allowed' = 'false'; 'model.unsaved_persistence_allowed' = 'false';
    'model.rerank_output_must_be_input_subset' = 'true'; 'model.implicit_provider_fallback_allowed' = 'false';
    'document.network_allowed' = 'false'; 'document.scripts_or_macros_allowed' = 'false';
    'document.shell_or_child_process_allowed' = 'false'; 'document.remote_resources_allowed' = 'false';
    'document.path_escape_allowed' = 'false'; 'document.automatic_download_allowed' = 'false';
    'document.automatic_upgrade_allowed' = 'false'; 'scale.in_place_schema_or_topology_change_allowed' = 'false';
    'scale.guarded_redb_route_switch_required' = 'true'; 'scale.qdrant_alias_is_commit' = 'false';
    'scale.old_route_pin_drain_required' = 'true'; 'scale.failed_candidate_discard_required' = 'true';
    'scale.post_switch_rollback_required' = 'true'; 'removal.baseline_restore_before_reclaim' = 'true';
    'removal.capability_draining_state_required' = 'true'; 'removal.worker_process_exit_required' = 'true';
    'removal.optional_cache_temp_cleanup_required' = 'true'; 'removal.route_pin_drain_required' = 'true';
    'removal.p15_regression_required' = 'true'; 'removal.claim_secure_erase' = 'false'
}
foreach ($entry in $lockedExpected.GetEnumerator()) {
    if (-not $fields.Contains($entry.Key)) { Add-Error "Missing locked W10 setting: $($entry.Key)"; continue }
    if ($fields[$entry.Key].Mode -cne 'LOCKED' -or $fields[$entry.Key].Default -cne $entry.Value) { Add-Error "Invalid locked W10 setting: $($entry.Key)" }
}
foreach ($key in @('gate.candidate_class', 'model.requested', 'document.requested', 'scale.requested')) {
    if (-not $fields.Contains($key) -or $fields[$key].Mode -cne 'TUNABLE' -or $fields[$key].Action -cne 'GATE_REQUIRED') { Add-Error "$key must be a GATE_REQUIRED tunable." }
}
$tunableBounds = [ordered]@{
    'model.max_batch_items' = @('1', '512'); 'model.max_batch_bytes' = @('1024', '268435456');
    'model.max_concurrency' = @('1', '16'); 'model.max_queue' = @('1', '256');
    'model.request_deadline_ms' = @('100', '300000'); 'model.cancel_grace_ms' = @('100', '30000');
    'document.max_input_bytes' = @('1024', '1073741824'); 'document.max_output_bytes' = @('1024', '1073741824');
    'document.max_pages' = @('1', '10000'); 'document.max_archive_members' = @('1', '100000');
    'document.max_nested_depth' = @('0', '8'); 'document.max_decompression_ratio' = @('1', '1000');
    'document.max_temp_bytes' = @('1048576', '4294967296'); 'document.max_concurrency' = @('1', '8');
    'document.request_deadline_ms' = @('1000', '1800000')
}
foreach ($entry in $tunableBounds.GetEnumerator()) {
    if (-not $fields.Contains($entry.Key)) { Add-Error "Missing W10 tunable: $($entry.Key)"; continue }
    $field = $fields[$entry.Key]
    if ($field.Mode -cne 'TUNABLE' -or $field.Min -cne $entry.Value[0] -or $field.Max -cne $entry.Value[1] -or $field.Action -cne 'APPLY_NEXT_WORKER_START') { Add-Error "Invalid W10 tunable: $($entry.Key)" }
}
Require-Tokens $paths.settings $text.settings @('[forbidden]', 'optional_depth_by_configuration = true', 'training_learning_or_persistent_content_cache = true', 'in_place_active_collection_change = true', 'self_review_or_self_acceptance = true')
Require-Tokens $paths.settings_doc $text.settings_doc @('LOCKED', 'TUNABLE', 'QUALIFIED_REF', 'configuration_alone_authorizes=false', 'No optional activation is `APPLY_LIVE`')

Require-Tokens $paths.optional_section $text.optional_section @('advanced_scale', 'scale_profile', 'apply_live_change` cannot activate', 'exact independently accepted P15')
$exampleOptional = Section $text.example 'optional_profiles'
foreach ($flag in @('semantic', 'document', 'advanced_scale')) { if (TBool $exampleOptional $flag) { Add-Error "Example optional flag $flag must remain false." } }
if ($exampleOptional -match '(?m)^\s*(model_provider_profile|document_provider_profile|scale_profile)\s*=') { Add-Error 'Example must not select optional profile refs.' }

$workflowFiles = @(Get-ChildItem (Join-Path $Root '.github/workflows') -Filter '*.yml' -File)
foreach ($file in $workflowFiles) {
    $workflowText = [IO.File]::ReadAllText($file.FullName)
    if ($workflowText -match '(?m)^\s*(pull_request|push|schedule):') { Add-Error "Automatic workflow trigger found in $($file.Name)." }
    if (-not $workflowText.Contains('workflow_dispatch:', [StringComparison]::Ordinal)) { Add-Error "Workflow $($file.Name) lacks workflow_dispatch." }
}
Require-Tokens $paths.workflow $text.workflow @('contents: read', 'persist-credentials: false', 'validate-w10-optional-depth.ps1')

$result = [ordered]@{
    ok = ($errors.Count -eq 0)
    owners = $ownerPackages.Count
    packets = $packetPackages.Count
    candidates = $candidateProfiles.Count
    probes = $probeIds.Count
    probes_per_candidate = 15
    g6_evidence_ids = $expectedEvidence.Count
    settings_fields = $fields.Count
    workflows = $workflowFiles.Count
    status = TStr $text.swarm 'status'
    selected_candidate = 'NONE'
    optional_depth = 'NOT_AUTHORIZED'
    warnings = @($warnings)
    errors = @($errors)
}

if ($Json) { $result | ConvertTo-Json -Depth 8 }
else {
    Write-Host 'ELIOT Search W10 optional-depth contract validation'
    Write-Host "owners=$($result.owners) packets=$($result.packets) candidates=$($result.candidates) probes=$($result.probes) settings=$($result.settings_fields) status=$($result.status)"
    foreach ($warning in $warnings) { Write-Warning $warning }
    foreach ($error in $errors) { Write-Host "ERROR: $error" -ForegroundColor Red }
    if ($result.ok) { Write-Host 'PASS' -ForegroundColor Green }
}
if (-not $result.ok) { exit 1 }
