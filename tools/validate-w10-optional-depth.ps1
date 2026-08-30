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

function TBool([string]$Text, [string]$Key, [bool]$Required = $true) {
    $match = [regex]::Match($Text, ('(?m)^{0}\s*=\s*(true|false)\s*$' -f [regex]::Escape($Key)))
    if (-not $match.Success) {
        if ($Required) { Add-Error "Missing TOML boolean '$Key'." }
        return $false
    }
    $match.Groups[1].Value -eq 'true'
}

function TInt([string]$Text, [string]$Key, [bool]$Required = $true) {
    $match = [regex]::Match($Text, ('(?m)^{0}\s*=\s*(-?\d+)\s*$' -f [regex]::Escape($Key)))
    if (-not $match.Success) {
        if ($Required) { Add-Error "Missing TOML integer '$Key'." }
        return [int64]0
    }
    [int64]$match.Groups[1].Value
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
    if (-not $match.Success) {
        Add-Error "Missing TOML section [$Name]."
        return ''
    }
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

function Require-Tokens([string]$RelativePath, [string]$Text, [string[]]$Tokens) {
    foreach ($token in $Tokens) {
        if ($Text.IndexOf($token, [StringComparison]::Ordinal) -lt 0) {
            Add-Error "$RelativePath is missing required token: $token"
        }
    }
}

function Validate-File([string]$Owner, [string]$RelativePath, [string]$Kind) {
    if ([string]::IsNullOrWhiteSpace($RelativePath)) { return }
    if (-not (Test-Path (Join-Path $Root $RelativePath) -PathType Leaf)) {
        Add-Error "$Owner references missing $Kind file: $RelativePath"
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
        if ($fields.Contains($key)) {
            Add-Error "Duplicate W10 settings field '$key'."
            continue
        }
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
    cross = 'docs/optional/W10_OPTIONAL_DEPTH_CONTRACTS_1.0.md'
    handoff = 'docs/handoff/W10_IMPLEMENTATION_PACKET.md'
    model = 'crates/search-model-provider/FUNCTIONS.md'
    model_worker = 'bins/eliot-search-model-worker/FUNCTIONS.md'
    document_worker = 'bins/eliot-search-doc-worker/FUNCTIONS.md'
    daemon = 'bins/eliot-searchd/W10_INTEGRATION.md'
    evaluation = 'crates/search-eval/W10_OPTIONAL_EVALUATION.md'
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
foreach ($entry in $paths.GetEnumerator()) {
    $text[$entry.Key] = Read-Required $entry.Value
}

$expectedPackages = @(
    'search-model-provider',
    'eliot-search-model-worker',
    'eliot-search-doc-worker',
    'eliot-searchd',
    'search-qdrant-bridge',
    'search-publication',
    'search-epoch-pins',
    'search-index-reclaimer',
    'search-eval'
)
$expectedScopes = [ordered]@{
    'search-model-provider' = 'crates/search-model-provider/**'
    'eliot-search-model-worker' = 'bins/eliot-search-model-worker/**'
    'eliot-search-doc-worker' = 'bins/eliot-search-doc-worker/**'
    'eliot-searchd' = 'bins/eliot-searchd/**'
    'search-qdrant-bridge' = 'crates/search-index-qdrant/search-qdrant-bridge/**'
    'search-publication' = 'crates/search-index-qdrant/search-publication/**'
    'search-epoch-pins' = 'crates/search-index-qdrant/search-epoch-pins/**'
    'search-index-reclaimer' = 'crates/search-index-qdrant/search-index-reclaimer/**'
    'search-eval' = 'crates/search-eval/**'
}
$expectedEvidence = @(
    'dedicated_optional_profile_adr',
    'exact_provider_artifact_qualification',
    'measured_material_benefit',
    'removal_or_uninstall_fallback',
    'migration_and_rollback_when_applicable'
)

# Manifest and owner closure.
if ((TStr $text.manifest 'status') -cne 'contract-only') {
    Add-Error 'W10 manifest must remain contract-only.'
}
foreach ($flag in @('implementation_authorized', 'optional_depth_authorized', 'provider_selected')) {
    if ((TBool $text.manifest $flag)) { Add-Error "W10 manifest cannot set $flag=true." }
}

$ownerBlocks = [regex]::Split($text.manifest, '(?m)^\[\[owner\]\]\s*$')
$ownerPackages = [System.Collections.Generic.List[string]]::new()
$ownerRoles = [System.Collections.Generic.List[string]]::new()
for ($i = 1; $i -lt $ownerBlocks.Count; $i++) {
    $block = $ownerBlocks[$i]
    $package = TStr $block 'package'
    $role = TStr $block 'role'
    if ($ownerPackages.Contains($package)) { Add-Error "Duplicate W10 owner: $package" }
    $ownerPackages.Add($package)
    $ownerRoles.Add($role)
    $functions = TStr $block 'functions' $false
    $contract = TStr $block 'contract' $false
    if (-not $functions -and -not $contract) { Add-Error "W10 owner $package lacks functions/contract." }
    Validate-File $package $functions 'functions'
    Validate-File $package $contract 'contract'
}
if (-not (Same-Set @($ownerPackages) $expectedPackages)) {
    Add-Error 'W10 manifest owner set is invalid.'
}
if (@($ownerRoles | Where-Object { $_ -ceq 'package' }).Count -ne 3) {
    Add-Error 'W10 must contain three ordinary package owners.'
}
if (@($ownerRoles | Where-Object { $_ -ceq 'integration' }).Count -ne 1) {
    Add-Error 'W10 must contain one integration owner.'
}
if (@($ownerRoles | Where-Object { $_ -ceq 'scale-package' }).Count -ne 4) {
    Add-Error 'W10 must contain four scale-package owners.'
}
if (@($ownerRoles | Where-Object { $_ -ceq 'evaluation-package' }).Count -ne 1) {
    Add-Error 'W10 must contain one evaluation-package owner.'
}

# Operation contract closure.
Require-Tokens $paths.cross $text.cross @(
    '## 2. Gate chain',
    '## 4. Model profile identity',
    '## 6. Model worker boundary',
    '## 9. Document worker no-execute boundary',
    '## 11. Measured material benefit',
    '## 12. Removal and baseline restoration',
    '## 14. P18 migration state machine',
    '## 18. Hard stop conditions',
    'G6: NOT ACCEPTED'
)
Require-Tokens $paths.model $text.model @(
    'validate_profile_descriptor', 'encode_documents', 'encode_query', 'rerank',
    'validate_rerank_output', 'classify_profile_capability',
    'validate_incremental_benefit_receipt', 'prepare_removal',
    'Typed failures', 'Required tests / qualification evidence'
)
Require-Tokens $paths.model_worker $text.model_worker @(
    'validate_startup', 'verify_inherited_containment', 'load_qualified_provider',
    'open_private_session', 'serve_encode', 'serve_rerank', 'cancel_request',
    'shutdown_and_remove', 'Typed failures', 'Required tests / qualification evidence'
)
Require-Tokens $paths.document_worker $text.document_worker @(
    'validate_provider_profile', 'verify_inherited_sandbox', 'inspect_container_and_input',
    'materialize', 'validate_materialization_output', 'cleanup_request_workspace',
    'shutdown_and_remove', 'Typed failures', 'Required tests / qualification evidence'
)
Require-Tokens $paths.daemon $text.daemon @(
    'evaluate_optional_candidate', 'plan_optional_activation', 'commit_optional_activation',
    'publish_optional_capability_snapshot', 'plan_optional_removal', 'commit_baseline_restore',
    'drain_and_remove_optional', 'plan_scale_candidate', 'commit_scale_route_switch',
    'rollback_scale_route', 'Typed failures', 'Required tests / evidence'
)
Require-Tokens $paths.evaluation $text.evaluation @(
    'validate_optional_campaign', 'freeze_candidate_comparison',
    'validate_candidate_fixture_extension', 'build_optional_trial_schedule',
    'ingest_candidate_operation_receipt', 'score_incremental_quality',
    'score_incremental_cost', 'audit_optional_noninterference',
    'validate_optional_fault_matrix', 'validate_removal_and_p15_regression',
    'compare_optional_candidate', 'build_g6_evidence_candidate',
    'verify_g6_independent_review', 'Typed failures',
    'Required tests / qualification evidence'
)
Require-Tokens $paths.bridge_scale $text.bridge_scale @(
    'probe_scale_capabilities', 'create_scale_candidate_collection',
    'validate_scale_query_equivalence', 'active collection schema/topology is never mutated in place'
)
Require-Tokens $paths.publication_scale $text.publication_scale @(
    'BASE_BUILT_AT_R0', 'FINAL_BARRIER_ENTERED', 'ROUTE_SWITCH_COMMITTED',
    'rollback_scale_intent'
)
Require-Tokens $paths.pins_scale $text.pins_scale @(
    'fence_old_route_for_new_pins', 'snapshot_route_drain', 'unknown/stale state fails closed'
)
Require-Tokens $paths.reclaimer_scale $text.reclaimer_scale @(
    'validate_retired_route_manifest', 'plan_old_route_reclaim',
    'ordinary route reclaim cannot satisfy purge'
)
Require-Tokens $paths.handoff $text.handoff @(
    'one candidate class/profile', 'Candidate evaluation boundary',
    'Required G6 evidence', 'Hard stop conditions', '45 DISABLED'
)

# Swarm packet closure.
if ((TStr $text.swarm 'status') -cne 'BLOCKED') { Add-Error 'W10 swarm packet must remain BLOCKED.' }
if ((TStr $text.swarm 'requires_accepted_gate') -cne 'G5') { Add-Error 'W10 must require accepted G5.' }
if ((TBool $text.swarm 'implementation_authorized')) { Add-Error 'W10 cannot authorize implementation.' }
if ((TBool $text.swarm 'optional_depth_authorized')) { Add-Error 'W10 cannot authorize optional depth.' }
if (-not (TBool $text.swarm 'one_candidate_per_ticket')) { Add-Error 'W10 must require one candidate per ticket.' }

$packetBlocks = [regex]::Split($text.swarm, '(?m)^\[\[packet\]\]\s*$')
$packetPackages = [System.Collections.Generic.List[string]]::new()
for ($i = 1; $i -lt $packetBlocks.Count; $i++) {
    $block = $packetBlocks[$i]
    $package = TStr $block 'package'
    if ($packetPackages.Contains($package)) { Add-Error "Duplicate W10 packet: $package" }
    $packetPackages.Add($package)
    $functions = TStr $block 'functions' $false
    $contract = TStr $block 'contract' $false
    if (-not $functions -and -not $contract) { Add-Error "W10 packet $package lacks functions/contract." }
    Validate-File $package $functions 'functions'
    Validate-File $package $contract 'contract'
    if (-not $expectedScopes.Contains($package)) {
        Add-Error "W10 packet has unexpected package: $package"
    } elseif ((TStr $block 'write_scope') -cne $expectedScopes[$package]) {
        Add-Error "W10 packet $package has wrong write scope."
    }
    foreach ($readPath in (TArray $block 'read_set')) {
        Validate-File $package $readPath 'read-set'
    }
    if ($package -eq 'search-eval') {
        $evalReads = @(TArray $block 'read_set')
        if ($evalReads -cnotcontains $paths.evaluation) {
            Add-Error 'W10 search-eval packet must include W10_OPTIONAL_EVALUATION.md.'
        }
        if ($evalReads -contains 'docs/handoff/W9_IMPLEMENTATION_PACKET.md') {
            Add-Error 'W10 search-eval packet must not replay the W9 implementation packet.'
        }
    }
}
if (-not (Same-Set @($packetPackages) $expectedPackages)) {
    Add-Error 'W10 swarm packet set differs from the manifest owner set.'
}
if (-not (Same-Set @(TArray $text.swarm 'required_evidence_ids') $expectedEvidence)) {
    Add-Error 'W10 required G6 evidence set is invalid.'
}

# Launch and central gate remain closed.
if ((TInt $text.launch 'active_wave') -ne 0) { Add-Error 'Launch wave must remain W0.' }
if ((TStr $text.launch 'active_stage') -cne 'P00') { Add-Error 'Launch phase must remain P00.' }
if (-not (Same-Set @(TArray $text.launch 'authorized_packages') @('search-contracts'))) {
    Add-Error 'Only search-contracts may remain authorized.'
}
$optionalDepth = Section $text.launch 'optional_depth'
if ((TStr $optionalDepth 'model_and_document_packages') -cne 'blocked') {
    Add-Error 'Model/document optional depth must remain blocked.'
}
if ((TStr $optionalDepth 'advanced_scale_packages') -cne 'blocked') {
    Add-Error 'Advanced scale must remain blocked.'
}
if ((TStr $optionalDepth 'packet') -cne $paths.swarm) { Add-Error 'Launch W10 packet path is inconsistent.' }
if ((TStr $optionalDepth 'qualification') -cne $paths.qualification) { Add-Error 'Launch W10 qualification path is inconsistent.' }
if ((TStr $optionalDepth 'settings') -cne $paths.settings) { Add-Error 'Launch W10 settings path is inconsistent.' }
if ((TStr $optionalDepth 'accepted_p15_receipt_ref') -ne '') { Add-Error 'Accepted P15 ref must remain empty.' }
if ((TStr $optionalDepth 'selected_candidate') -cne 'NONE') { Add-Error 'No W10 candidate may be selected.' }
if ((TArray $optionalDepth 'accepted_candidate_receipts').Count -ne 0) { Add-Error 'No W10 candidate receipt may be accepted.' }
if ((TStr $optionalDepth 'requires_accepted_gate') -cne 'G5') { Add-Error 'W10 launch must require G5.' }
if ((TBool $optionalDepth 'configuration_alone_authorizes')) { Add-Error 'Configuration cannot authorize W10.' }

$g6Match = [regex]::Match($text.gates, '(?ms)^\[\[gate\]\]\s*id\s*=\s*"G6"(.*?)(?=^\[\[gate\]\]|\z)')
if (-not $g6Match.Success) {
    Add-Error 'Central G6 gate block is missing.'
} elseif (-not (Same-Set @(TArray $g6Match.Groups[1].Value 'required_evidence') $expectedEvidence)) {
    Add-Error 'Central G6 evidence differs from W10.'
}

# Baseline authority floors.
if ((TStr $text.baseline 'status') -cne 'DISABLED_NOT_SELECTED') {
    Add-Error 'W10 baseline must remain DISABLED_NOT_SELECTED.'
}
foreach ($flag in @('implementation_authorized', 'optional_depth_authorized')) {
    if ((TBool $text.baseline $flag)) { Add-Error "W10 baseline cannot set $flag=true." }
}
$acceptedBaseline = Section $text.baseline 'accepted_baseline'
$candidate = Section $text.baseline 'candidate'
$activation = Section $text.baseline 'activation'
$common = Section $text.baseline 'common_policy'
$selection = Section $text.baseline 'selection'
if ((TStr $acceptedBaseline 'p15_report_ref') -cne 'UNSELECTED') { Add-Error 'P15 baseline ref must remain UNSELECTED.' }
if ((TStr $candidate 'profile_class') -cne 'NONE') { Add-Error 'Candidate class must remain NONE.' }
if ((TStr $candidate 'profile_ref') -cne 'UNSELECTED') { Add-Error 'Candidate profile must remain UNSELECTED.' }
if ((TStr $candidate 'status') -cne 'DISABLED') { Add-Error 'Candidate status must remain DISABLED.' }
foreach ($flag in @(
    'compiled_feature_present', 'explicit_configuration_enabled', 'binding_authorized',
    'worker_qualified', 'candidate_route_validated', 'gate_receipt_accepted', 'active'
)) {
    if ((TBool $activation $flag)) { Add-Error "Premature activation flag: $flag" }
}
foreach ($flag in @(
    'network_allowed', 'automatic_download_allowed', 'automatic_upgrade_allowed',
    'training_or_learning_allowed', 'persistent_content_cache_allowed',
    'unsaved_persistence_allowed', 'generative_answer_authority_allowed',
    'client_admission_authority_allowed', 'provider_output_is_source_evidence',
    'in_place_schema_change_allowed', 'qdrant_alias_is_commit',
    'self_review_allowed', 'self_acceptance_allowed'
)) {
    if ((TBool $common $flag)) { Add-Error "Unsafe W10 common flag enabled: $flag" }
}
foreach ($flag in @(
    'latest_allowed', 'version_range_allowed', 'floating_revision_allowed',
    'documentation_only_acceptance_allowed', 'unit_tests_only_acceptance_allowed',
    'compilation_only_acceptance_allowed', 'configuration_only_activation_allowed'
)) {
    if ((TBool $selection $flag)) { Add-Error "Unsafe W10 selection flag enabled: $flag" }
}

# Candidate profile templates remain unselected and non-executing.
if ((TStr $text.model_profile 'status') -cne 'UNSELECTED') { Add-Error 'Model profile must remain UNSELECTED.' }
if ((TBool $text.model_profile 'enabled')) { Add-Error 'Model profile must remain disabled.' }
$modelCaps = Section $text.model_profile 'capabilities'
foreach ($flag in @(
    'rerank_only', 'dense_vector', 'multivector', 'generative_answers',
    'training_or_learning', 'network', 'automatic_download',
    'automatic_upgrade', 'persistent_input_cache'
)) {
    if ((TBool $modelCaps $flag)) { Add-Error "Premature model capability: $flag" }
}
$modelArtifact = Section $text.model_profile 'artifact'
foreach ($key in @(
    'provider_name', 'provider_source_ref', 'provider_version', 'model_name',
    'model_source_ref', 'runtime_backend', 'runtime_version', 'Windows_package_ref'
)) {
    if ((TStr $modelArtifact $key) -cne 'UNSELECTED') { Add-Error "Model artifact $key must remain UNSELECTED." }
}

if ((TStr $text.document_profile 'status') -cne 'UNSELECTED') { Add-Error 'Document profile must remain UNSELECTED.' }
if ((TBool $text.document_profile 'enabled')) { Add-Error 'Document profile must remain disabled.' }
$docArtifact = Section $text.document_profile 'artifact'
foreach ($key in @(
    'provider_name', 'provider_source_ref', 'provider_version',
    'runtime_name', 'runtime_version', 'Windows_package_ref'
)) {
    if ((TStr $docArtifact $key) -cne 'UNSELECTED') { Add-Error "Document artifact $key must remain UNSELECTED." }
}
if ((TBool $docArtifact 'Python_or_Node_runtime')) { Add-Error 'Document Python/Node runtime cannot be selected.' }
$docSecurity = Section $text.document_profile 'security'
foreach ($flag in @(
    'network_allowed', 'scripts_allowed', 'javascript_allowed', 'macros_allowed',
    'ole_actions_allowed', 'hooks_or_filters_allowed', 'shell_or_child_process_allowed',
    'remote_resources_allowed', 'credential_prompts_allowed', 'path_traversal_allowed',
    'symlink_hardlink_reparse_escape_allowed', 'automatic_download', 'automatic_upgrade'
)) {
    if ((TBool $docSecurity $flag)) { Add-Error "Unsafe document flag enabled: $flag" }
}

if ((TStr $text.scale_profile 'status') -cne 'UNSELECTED') { Add-Error 'Scale profile must remain UNSELECTED.' }
if ((TBool $text.scale_profile 'enabled')) { Add-Error 'Scale profile must remain disabled.' }
$scaleArtifact = Section $text.scale_profile 'artifact'
foreach ($key in @('qdrant_server_version', 'qdrant_client_version', 'Windows_package_ref')) {
    if ((TStr $scaleArtifact $key) -cne 'UNSELECTED') { Add-Error "Scale artifact $key must remain UNSELECTED." }
}
$topology = Section $text.scale_profile 'topology'
if ((TStr $topology 'profile_name') -cne 'UNSELECTED') { Add-Error 'Scale topology profile must remain UNSELECTED.' }
if (-not (TBool $topology 'strict_mode_required')) { Add-Error 'Scale strict mode must remain required.' }
if ((TBool $topology 'in_place_change_allowed')) { Add-Error 'Scale in-place change must remain forbidden.' }
if ((TBool $topology 'qdrant_alias_is_commit')) { Add-Error 'Qdrant alias cannot be a Search commit.' }
$migration = Section $text.scale_profile 'migration'
foreach ($flag in @(
    'base_at_r0_required', 'ordered_catch_up_required', 'final_barrier_at_r1_required',
    'guarded_redb_route_switch_required', 'old_route_pins_required',
    'failed_candidate_discard_required', 'post_switch_rollback_required'
)) {
    if (-not (TBool $migration $flag)) { Add-Error "Scale migration requirement disabled: $flag" }
}

# Probe templates and G6 map.
if ((TStr $text.probes 'status') -cne 'TEMPLATES_DISABLED') { Add-Error 'W10 probes must remain disabled templates.' }
if ((TStr $text.probes 'selected_candidate') -cne 'NONE') { Add-Error 'W10 probes cannot select a candidate.' }
if (-not (TBool $text.probes 'raw_output_required_for_pass')) { Add-Error 'Raw output must be required.' }
if (-not (TBool $text.probes 'independent_review_required_for_pass')) { Add-Error 'Independent review must be required.' }
$probeDefaults = Section $text.probes 'probe_defaults'
if (-not (TBool $probeDefaults 'mandatory_when_selected')) { Add-Error 'Selected candidate probes must be mandatory.' }
if ((TStr $probeDefaults 'result') -cne 'DISABLED') { Add-Error 'Probe default result must remain DISABLED.' }
foreach ($key in @('raw_output_ref', 'raw_output_digest', 'reviewer_receipt_ref')) {
    if ((TStr $probeDefaults $key) -ne '') { Add-Error "Probe default $key must remain empty." }
}

$probeBlocks = [regex]::Split($text.probes, '(?m)^\[\[probe\]\]\s*$')
$probeIds = [System.Collections.Generic.HashSet[string]]::new([StringComparer]::Ordinal)
$profileProbeIds = [ordered]@{}
$profileEvidenceCounts = [ordered]@{}
foreach ($profile in @('model', 'document', 'scale')) {
    $profileProbeIds[$profile] = [System.Collections.Generic.List[string]]::new()
    $profileEvidenceCounts[$profile] = [ordered]@{}
    foreach ($evidence in $expectedEvidence) { $profileEvidenceCounts[$profile][$evidence] = 0 }
}
for ($i = 1; $i -lt $probeBlocks.Count; $i++) {
    $block = $probeBlocks[$i]
    $id = TStr $block 'id'
    $profile = TStr $block 'profile'
    $evidence = TStr $block 'evidence_id'
    if (-not $probeIds.Add($id)) { Add-Error "Duplicate W10 probe: $id" }
    if (-not $profileProbeIds.Contains($profile)) {
        Add-Error "Probe $id has unknown profile $profile."
    } else {
        $profileProbeIds[$profile].Add($id)
        if (-not $profileEvidenceCounts[$profile].Contains($evidence)) {
            Add-Error "Probe $id has unknown evidence $evidence."
        } else {
            $profileEvidenceCounts[$profile][$evidence]++
        }
    }
    [void](TStr $block 'producer')
    [void](TStr $block 'purpose')
    $result = TStr $block 'result' $false
    if ($result -and $result -cne 'DISABLED') { Add-Error "Probe $id has premature result $result." }
    foreach ($key in @('raw_output_ref', 'raw_output_digest', 'reviewer_receipt_ref')) {
        if ((TStr $block $key $false)) { Add-Error "Probe $id contains premature $key." }
    }
}
if ($probeIds.Count -ne 45) { Add-Error "Expected 45 W10 probes; parsed $($probeIds.Count)." }
foreach ($profile in @('model', 'document', 'scale')) {
    if ($profileProbeIds[$profile].Count -ne 15) { Add-Error "$profile must have 15 probes." }
    foreach ($evidence in $expectedEvidence) {
        if ($profileEvidenceCounts[$profile][$evidence] -ne 3) {
            Add-Error "$profile/$evidence must have exactly three probes."
        }
    }
}

if ((TStr $text.gate_map 'status') -cne 'CANDIDATE_TEMPLATES_DISABLED') {
    Add-Error 'W10 gate map must remain disabled.'
}
if ((TStr $text.gate_map 'selected_candidate') -cne 'NONE') {
    Add-Error 'W10 gate map cannot select a candidate.'
}
$candidateBlocks = [regex]::Split($text.gate_map, '(?m)^\[\[candidate\]\]\s*$')
$candidateProfiles = [System.Collections.Generic.List[string]]::new()
for ($i = 1; $i -lt $candidateBlocks.Count; $i++) {
    $block = $candidateBlocks[$i]
    $profile = TStr $block 'profile'
    $candidateProfiles.Add($profile)
    if ((TStr $block 'status') -cne 'DISABLED') { Add-Error "$profile gate map must remain DISABLED." }
    $mapped = [System.Collections.Generic.List[string]]::new()
    foreach ($evidence in $expectedEvidence) {
        $ids = @(TArray $block $evidence)
        if ($ids.Count -ne 3) { Add-Error "$profile/$evidence must map three probes." }
        foreach ($id in $ids) {
            $mapped.Add($id)
            if (-not $probeIds.Contains($id)) { Add-Error "Gate map references unknown probe $id." }
        }
    }
    if ($profileProbeIds.Contains($profile)) {
        if (-not (Same-Set @($mapped) @($profileProbeIds[$profile]))) {
            Add-Error "$profile gate map does not cover exactly its probe set."
        }
    }
}
if (-not (Same-Set @($candidateProfiles) @('model', 'document', 'scale'))) {
    Add-Error 'W10 gate-map profile set is invalid.'
}

# Settings and safe example.
if ((TStr $text.settings 'status') -cne 'schema-only') { Add-Error 'W10 settings must remain schema-only.' }
if ((TBool $text.settings 'implementation_authorized')) { Add-Error 'W10 settings cannot authorize implementation.' }
if ((TBool $text.settings 'optional_depth_authorized')) { Add-Error 'W10 settings cannot authorize optional depth.' }
$fields = Parse-Fields $text.settings
$qualifiedRefs = @(
    'gate.accepted_p15_receipt_ref', 'gate.candidate_adr_ref',
    'gate.candidate_qualification_ref', 'gate.candidate_benefit_ref',
    'gate.candidate_removal_ref', 'gate.candidate_migration_rollback_ref',
    'model.profile_ref', 'document.profile_ref', 'scale.profile_ref',
    'scale.measured_bottleneck_ref'
)
foreach ($key in $qualifiedRefs) {
    if (-not $fields.Contains($key)) {
        Add-Error "Missing W10 qualified ref: $key"
    } elseif ($fields[$key].Mode -cne 'QUALIFIED_REF' -or $fields[$key].Default -cne '"UNSELECTED"') {
        Add-Error "$key must be an UNSELECTED QUALIFIED_REF."
    }
}
$lockedExpected = [ordered]@{
    'gate.compiled_feature_required' = 'true'
    'gate.explicit_configuration_required' = 'true'
    'gate.binding_authorization_required' = 'true'
    'gate.configuration_alone_authorizes' = 'false'
    'gate.one_candidate_per_ticket' = 'true'
    'model.network_allowed' = 'false'
    'model.automatic_download_allowed' = 'false'
    'model.automatic_upgrade_allowed' = 'false'
    'model.training_or_learning_allowed' = 'false'
    'model.generative_answer_allowed' = 'false'
    'model.persistent_input_cache_allowed' = 'false'
    'model.unsaved_persistence_allowed' = 'false'
    'model.rerank_output_must_be_input_subset' = 'true'
    'model.implicit_provider_fallback_allowed' = 'false'
    'document.network_allowed' = 'false'
    'document.scripts_or_macros_allowed' = 'false'
    'document.shell_or_child_process_allowed' = 'false'
    'document.remote_resources_allowed' = 'false'
    'document.path_escape_allowed' = 'false'
    'document.automatic_download_allowed' = 'false'
    'document.automatic_upgrade_allowed' = 'false'
    'scale.in_place_schema_or_topology_change_allowed' = 'false'
    'scale.guarded_redb_route_switch_required' = 'true'
    'scale.qdrant_alias_is_commit' = 'false'
    'scale.old_route_pin_drain_required' = 'true'
    'scale.failed_candidate_discard_required' = 'true'
    'scale.post_switch_rollback_required' = 'true'
    'removal.baseline_restore_before_reclaim' = 'true'
    'removal.capability_draining_state_required' = 'true'
    'removal.worker_process_exit_required' = 'true'
    'removal.optional_cache_temp_cleanup_required' = 'true'
    'removal.route_pin_drain_required' = 'true'
    'removal.p15_regression_required' = 'true'
    'removal.claim_secure_erase' = 'false'
}
foreach ($entry in $lockedExpected.GetEnumerator()) {
    if (-not $fields.Contains($entry.Key)) {
        Add-Error "Missing locked W10 setting: $($entry.Key)"
    } elseif ($fields[$entry.Key].Mode -cne 'LOCKED' -or $fields[$entry.Key].Default -cne $entry.Value) {
        Add-Error "Invalid locked W10 setting: $($entry.Key)"
    }
}
foreach ($key in @('gate.candidate_class', 'model.requested', 'document.requested', 'scale.requested')) {
    if (-not $fields.Contains($key)) {
        Add-Error "Missing W10 gate tunable: $key"
    } elseif ($fields[$key].Mode -cne 'TUNABLE' -or $fields[$key].Action -cne 'GATE_REQUIRED') {
        Add-Error "$key must be a GATE_REQUIRED tunable."
    }
}
Require-Tokens $paths.settings $text.settings @(
    '[forbidden]', 'optional_depth_by_configuration = true',
    'training_learning_or_persistent_content_cache = true',
    'in_place_active_collection_change = true',
    'self_review_or_self_acceptance = true'
)
Require-Tokens $paths.settings_doc $text.settings_doc @(
    'LOCKED', 'TUNABLE', 'QUALIFIED_REF',
    'configuration_alone_authorizes=false', 'No optional activation is `APPLY_LIVE`'
)
Require-Tokens $paths.optional_section $text.optional_section @(
    'advanced_scale', 'scale_profile', 'apply_live_change` cannot activate',
    'exact independently accepted P15'
)
$exampleOptional = Section $text.example 'optional_profiles'
foreach ($flag in @('semantic', 'document', 'advanced_scale')) {
    if ((TBool $exampleOptional $flag)) { Add-Error "Example optional flag $flag must remain false." }
}
if ($exampleOptional -match '(?m)^\s*(model_provider_profile|document_provider_profile|scale_profile)\s*=') {
    Add-Error 'Example must not select optional profile refs.'
}

# Repository workflow policy.
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
Require-Tokens $paths.workflow $text.workflow @(
    'contents: read', 'persist-credentials: false', 'validate-w10-optional-depth.ps1'
)

$result = [ordered]@{
    ok = ($errors.Count -eq 0)
    owners = $ownerPackages.Count
    packets = $packetPackages.Count
    candidates = $candidateProfiles.Count
    probes = $probeIds.Count
    g6_evidence_ids = $expectedEvidence.Count
    settings_fields = $fields.Count
    workflows = $workflowFiles.Count
    status = TStr $text.swarm 'status'
    selected_candidate = 'NONE'
    optional_depth = 'NOT_AUTHORIZED'
    warnings = @($warnings)
    errors = @($errors)
}

if ($Json) {
    $result | ConvertTo-Json -Depth 8
} else {
    Write-Host 'ELIOT Search W10 optional-depth validation'
    Write-Host "owners=$($result.owners) packets=$($result.packets) candidates=$($result.candidates) probes=$($result.probes) settings=$($result.settings_fields) status=$($result.status)"
    foreach ($warning in $warnings) { Write-Warning $warning }
    foreach ($error in $errors) { Write-Host "ERROR: $error" -ForegroundColor Red }
    if ($result.ok) { Write-Host 'PASS' -ForegroundColor Green }
}

if (-not $result.ok) { exit 1 }
