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

function Get-String([string]$Text, [string]$Key, [bool]$Required = $true) {
    $pattern = '(?m)^{0}\s*=\s*"([^"]*)"\s*$' -f [regex]::Escape($Key)
    $match = [regex]::Match($Text, $pattern)
    if (-not $match.Success) {
        if ($Required) { Fail "Missing string: $Key" }
        return ''
    }
    $match.Groups[1].Value
}

function Get-Int([string]$Text, [string]$Key, [bool]$Required = $true) {
    $pattern = '(?m)^{0}\s*=\s*(-?\d+)\s*$' -f [regex]::Escape($Key)
    $match = [regex]::Match($Text, $pattern)
    if (-not $match.Success) {
        if ($Required) { Fail "Missing integer: $Key" }
        return [int64]0
    }
    [int64]$match.Groups[1].Value
}

function Get-Bool([string]$Text, [string]$Key, [bool]$Required = $true) {
    $pattern = '(?m)^{0}\s*=\s*(true|false)\s*$' -f [regex]::Escape($Key)
    $match = [regex]::Match($Text, $pattern)
    if (-not $match.Success) {
        if ($Required) { Fail "Missing boolean: $Key" }
        return $false
    }
    $match.Groups[1].Value -eq 'true'
}

function Get-Array([string]$Text, [string]$Key) {
    $pattern = '(?ms)^{0}\s*=\s*\[(.*?)\]' -f [regex]::Escape($Key)
    $match = [regex]::Match($Text, $pattern)
    if (-not $match.Success) { return @() }
    @(
        [regex]::Matches($match.Groups[1].Value, '"([^"\r\n]+)"') |
            ForEach-Object { $_.Groups[1].Value }
    )
}

function Get-Section([string]$Text, [string]$Name) {
    $pattern = '(?ms)^\[{0}\]\s*(.*?)(?=^\[|\z)' -f [regex]::Escape($Name)
    $match = [regex]::Match($Text, $pattern)
    if (-not $match.Success) {
        Fail "Missing section: $Name"
        return ''
    }
    $match.Groups[1].Value
}

function Same-Set([object[]]$Left, [object[]]$Right) {
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

function Require-Tokens([string]$Path, [string]$Text, [string[]]$Tokens) {
    foreach ($token in $Tokens) {
        if ($Text.IndexOf($token, [StringComparison]::Ordinal) -lt 0) {
            Fail "$Path lacks: $token"
        }
    }
}

function Assert-WorkflowPolicy([string]$Path, [string]$Text) {
    if (-not [regex]::IsMatch($Text, '(?m)^\s{2}workflow_dispatch:\s*$')) {
        Fail "$Path lacks workflow_dispatch."
    }
    $forbidden = '(?m)^\s{0,6}(push|pull_request|pull_request_target|merge_group|schedule|workflow_run|repository_dispatch|workflow_call|release|issues|issue_comment|discussion|discussion_comment|create|delete|branch_protection_rule|check_run|check_suite|deployment|deployment_status|fork|gollum|label|milestone|page_build|project|project_card|project_column|public|registry_package|status|watch):\s*$'
    if ([regex]::IsMatch($Text, $forbidden)) {
        Fail "$Path contains an automatic or externally chained trigger."
    }
    if (-not [regex]::IsMatch($Text, '(?m)^\s{2}contents:\s*read\s*$')) {
        Fail "$Path must declare contents: read."
    }
    if ($Text.IndexOf('persist-credentials: false', [StringComparison]::Ordinal) -lt 0) {
        Fail "$Path must disable checkout credential persistence."
    }
}

function Assert-EmptyControlDirectory([string]$Path) {
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

$manifestPath = 'qualification/ticket-issuance/manifest.toml'
$baselinePath = 'qualification/ticket-issuance/baseline.toml'
$fixturesPath = 'qualification/ticket-issuance/fixtures.toml'
$probesPath = 'qualification/ticket-issuance/probes.toml'
$qualificationPath = 'qualification/ticket-issuance/TICKET_ISSUANCE_QUALIFICATION.md'
$operationsPath = 'swarm/control-plane-operations.toml'
$controlSchemaPath = 'swarm/control-plane-schema.toml'
$typesPath = 'swarm/schemas/types-v1.toml'
$orchestrationPath = 'swarm/orchestration.toml'
$launchPath = 'swarm/launch-state.toml'
$contextDraftManifestPath = 'swarm/context-drafts/manifest.toml'
$workflowPath = '.github/workflows/ticket-issuance-conformance.yml'

$manifest = Read-File $manifestPath
$baseline = Read-File $baselinePath
$fixtures = Read-File $fixturesPath
$probes = Read-File $probesPath
$qualification = Read-File $qualificationPath
$operations = Read-File $operationsPath
$controlSchema = Read-File $controlSchemaPath
$types = Read-File $typesPath
$orchestration = Read-File $orchestrationPath
$launch = Read-File $launchPath
$contextDraftManifest = Read-File $contextDraftManifestPath
$workflow = Read-File $workflowPath

$expectedPaths = [ordered]@{
    qualification = $qualificationPath
    baseline = $baselinePath
    fixture_registry = $fixturesPath
    probe_registry = $probesPath
    operation_registry = $operationsPath
    record_schema_registry = $controlSchemaPath
    type_registry = $typesPath
    validator = 'tools/validate-ticket-issuance-conformance.ps1'
    manual_workflow = $workflowPath
}

if ((Get-Int $manifest 'schema_version') -ne 1 -or (Get-String $manifest 'status') -cne 'DESIGNED_NOT_EXECUTED' -or (Get-String $manifest 'owner') -cne 'integration-owner') {
    Fail 'Qualification manifest identity mismatch.'
}
foreach ($entry in $expectedPaths.GetEnumerator()) {
    if ((Get-String $manifest $entry.Key) -cne $entry.Value) {
        Fail "Qualification manifest path mismatch: $($entry.Key)"
    }
}
if ((Get-String $manifest 'workflow_policy') -cne 'manual_only') {
    Fail 'Qualification workflow policy must be manual_only.'
}
$expectedCounts = [ordered]@{
    record_schema_count = 8
    operation_count = 12
    fixture_count = 52
    probe_count = 64
    failure_code_count = 31
    recovery_disposition_count = 4
}
foreach ($entry in $expectedCounts.GetEnumerator()) {
    if ((Get-Int $manifest $entry.Key) -ne $entry.Value) {
        Fail "Qualification manifest count mismatch: $($entry.Key)"
    }
}
if (-not (Get-Bool $manifest 'all_probes_mandatory') -or (Get-String $manifest 'all_probe_results_initial') -cne 'UNAVAILABLE' -or (Get-String $manifest 'prose_only_evidence') -cne 'reject') {
    Fail 'Qualification manifest evidence floor mismatch.'
}

$authority = Get-Section $manifest 'authority'
foreach ($unsafe in @('produces_materialized_context', 'issues_assignment_ticket', 'issues_writer_lease', 'records_writer_acknowledgement', 'records_package_submission', 'records_independent_review', 'publishes_package_handoff', 'accepts_gate', 'accepts_wave', 'advances_launch_state')) {
    if (Get-Bool $authority $unsafe) { Fail "Qualification manifest creates authority: $unsafe" }
}
$content = Get-Section $manifest 'content'
foreach ($unsafe in @('real_control_record_instances_allowed', 'real_actor_assignment_allowed', 'real_secret_or_source_content_allowed', 'absolute_local_paths_allowed', 'mutable_git_refs_allowed')) {
    if (Get-Bool $content $unsafe) { Fail "Qualification manifest permits unsafe content: $unsafe" }
}
if (-not (Get-Bool $content 'synthetic_fixture_descriptors_only')) {
    Fail 'Qualification fixtures must remain synthetic descriptors only.'
}
$evidence = Get-Section $manifest 'evidence'
foreach ($required in @('raw_output_required_for_PASS', 'independent_reviewer_receipt_required_for_PASS')) {
    if (-not (Get-Bool $evidence $required)) { Fail "Qualification evidence floor disabled: $required" }
}
foreach ($unsafe in @('UNAVAILABLE_may_be_inferred_as_PASS', 'structural_validation_is_runtime_evidence', 'workflow_success_is_authority')) {
    if (Get-Bool $evidence $unsafe) { Fail "Qualification evidence overclaim enabled: $unsafe" }
}

if ((Get-Int $types 'schema_version') -ne 2 -or (Get-String $types 'registry_kind') -cne 'control_plane_types_v1') {
    Fail 'Type registry identity mismatch.'
}
$typeBlocks = [regex]::Split($types, '(?m)^\[\[type\]\]\s*$')
$typeMap = @{}
for ($i = 1; $i -lt $typeBlocks.Count; $i++) {
    $name = Get-String $typeBlocks[$i] 'name'
    if ($typeMap.ContainsKey($name)) { Fail "Duplicate type: $name" }
    else { $typeMap[$name] = $typeBlocks[$i] }
}
if (-not $typeMap.ContainsKey('ClosedReasonCode')) {
    Fail 'Type registry lacks ClosedReasonCode.'
}
$failureCodes = @(Get-Array ([string]$typeMap['ClosedReasonCode']) 'allowed')
if ($failureCodes.Count -ne 31) {
    Fail "ClosedReasonCode count $($failureCodes.Count) != 31."
}

if ((Get-Int $operations 'schema_version') -ne 1 -or (Get-String $operations 'status') -cne 'CONTRACT_ONLY_NOT_IMPLEMENTED' -or (Get-String $operations 'owner') -cne 'integration-owner') {
    Fail 'Operation registry identity mismatch.'
}
foreach ($entry in @(
    @('operation_contract', 'docs/handoff/TICKET_ISSUANCE_OPERATIONS.md'),
    @('record_schema_registry', $controlSchemaPath),
    @('type_registry', $typesPath),
    @('orchestration_registry', $orchestrationPath),
    @('qualification_manifest', $manifestPath),
    @('validator', 'tools/validate-ticket-issuance-conformance.ps1'),
    @('manual_workflow', $workflowPath)
)) {
    if ((Get-String $operations $entry[0]) -cne $entry[1]) {
        Fail "Operation registry path mismatch: $($entry[0])"
    }
}
if ((Get-String $operations 'workflow_policy') -cne 'manual_only' -or (Get-String $operations 'unknown_operations') -cne 'reject' -or (Get-String $operations 'unknown_failure_codes') -cne 'reject') {
    Fail 'Operation registry does not fail closed.'
}
if ((Get-Int $operations 'operation_count') -ne 12 -or (Get-Int $operations 'pure_operation_count') -ne 2 -or (Get-Int $operations 'mutation_operation_count') -ne 9 -or (Get-Int $operations 'read_only_recovery_operation_count') -ne 1) {
    Fail 'Operation class counts mismatch.'
}
$operationKinds = @(Get-Array $operations 'operation_kinds')
$recoveryDispositions = @(Get-Array $operations 'recovery_dispositions')
if (-not (Same-Sequence $operationKinds @('PURE', 'MUTATION', 'READ_ONLY_RECOVERY'))) {
    Fail 'Operation kind registry mismatch.'
}
if (-not (Same-Sequence $recoveryDispositions @('RECOVERED_SUCCEEDED', 'SAFE_TO_RETRY', 'CONFLICT', 'PRESERVE_OUTCOME_UNKNOWN'))) {
    Fail 'Recovery disposition registry mismatch.'
}

$operationBlocks = [regex]::Split($operations, '(?m)^\[\[operation\]\]\s*$')
$operationMap = [ordered]@{}
$operationFailures = [System.Collections.Generic.HashSet[string]]::new([StringComparer]::Ordinal)
$operationDomains = [System.Collections.Generic.HashSet[string]]::new([StringComparer]::Ordinal)
$pureIds = [System.Collections.Generic.List[string]]::new()
$mutationIds = [System.Collections.Generic.List[string]]::new()
$recoveryIds = [System.Collections.Generic.List[string]]::new()
for ($i = 1; $i -lt $operationBlocks.Count; $i++) {
    $block = $operationBlocks[$i]
    $id = Get-String $block 'id'
    $kind = Get-String $block 'kind'
    if ($operationMap.Contains($id)) { Fail "Duplicate operation: $id"; continue }
    $operationMap[$id] = $block
    if ($operationKinds -notcontains $kind) { Fail "$id has unknown operation kind $kind." }
    if ([string]::IsNullOrWhiteSpace((Get-String $block 'authority')) -or [string]::IsNullOrWhiteSpace((Get-String $block 'actor'))) {
        Fail "$id lacks authority/actor."
    }
    if (@(Get-Array $block 'input_kinds').Count -eq 0 -or @(Get-Array $block 'output_kinds').Count -eq 0) {
        Fail "$id lacks bounded input/output kinds."
    }
    $domain = Get-String $block 'domain_separator'
    $failures = @(Get-Array $block 'failure_codes')
    if ($failures.Count -eq 0) { Fail "$id lacks typed failure ownership." }
    foreach ($failure in $failures) {
        if ($failureCodes -notcontains $failure) { Fail "$id references unknown failure $failure." }
        [void]$operationFailures.Add($failure)
    }
    if ([string]::IsNullOrWhiteSpace((Get-String $block 'idempotency')) -or [string]::IsNullOrWhiteSpace((Get-String $block 'cancellation')) -or [string]::IsNullOrWhiteSpace((Get-String $block 'deadline')) -or [string]::IsNullOrWhiteSpace((Get-String $block 'recovery')) -or [string]::IsNullOrWhiteSpace((Get-String $block 'state_transition'))) {
        Fail "$id lacks operation semantics."
    }
    switch ($kind) {
        'PURE' {
            [void]$pureIds.Add($id)
            if ($domain -ne '') { Fail "$id is pure but has a mutation domain separator." }
            if ((Get-String $block 'recovery') -cne 'not_applicable_no_mutation') { Fail "$id pure recovery semantics mismatch." }
        }
        'MUTATION' {
            [void]$mutationIds.Add($id)
            if ([string]::IsNullOrWhiteSpace($domain) -or -not $operationDomains.Add($domain)) { Fail "$id lacks a unique mutation domain separator." }
            if ((Get-String $block 'deadline') -cne 'finite_required') { Fail "$id mutation deadline must be finite_required." }
        }
        'READ_ONLY_RECOVERY' {
            [void]$recoveryIds.Add($id)
            if ([string]::IsNullOrWhiteSpace($domain) -or -not $operationDomains.Add($domain)) { Fail "$id lacks a unique recovery domain separator." }
            if ((Get-String $block 'recovery') -cne 'not_applicable_this_is_recovery') { Fail "$id recovery operation semantics mismatch." }
        }
    }
}
if ($operationMap.Count -ne 12 -or $pureIds.Count -ne 2 -or $mutationIds.Count -ne 9 -or $recoveryIds.Count -ne 1) {
    Fail 'Parsed operation counts mismatch.'
}
if (-not (Same-Set $operationFailures.ToArray() $failureCodes)) {
    Fail 'Operation failure ownership does not cover exactly ClosedReasonCode.'
}

$operationInvariants = Get-Section $operations 'invariants'
foreach ($required in @('mutation_operations_require_operation_id', 'mutation_operations_require_exact_post_write_readback')) {
    if (-not (Get-Bool $operationInvariants $required)) { Fail "Operation invariant disabled: $required" }
}
foreach ($unsafe in @('pure_operations_create_records', 'blind_retry_after_possible_write_allowed', 'partial_multi_record_success_allowed', 'control_record_in_place_mutation_allowed', 'package_writer_may_publish_integration_records', 'package_handoff_may_advance_gate_or_wave', 'workflow_result_may_issue_authority')) {
    if (Get-Bool $operationInvariants $unsafe) { Fail "Unsafe operation invariant: $unsafe" }
}
if ((Get-String $operationInvariants 'same_operation_different_input') -cne 'CONTROL_OPERATION_CONFLICT') {
    Fail 'Operation conflict invariant mismatch.'
}

if ((Get-Int $baseline 'schema_version') -ne 1 -or (Get-String $baseline 'status') -cne 'DESIGNED_NOT_EXECUTED') {
    Fail 'Qualification baseline identity mismatch.'
}
foreach ($entry in @(
    @('qualification_manifest', $manifestPath),
    @('operation_registry', $operationsPath),
    @('record_schema_registry', $controlSchemaPath),
    @('type_registry', $typesPath)
)) {
    if ((Get-String $baseline $entry[0]) -cne $entry[1]) { Fail "Baseline path mismatch: $($entry[0])" }
}
$baselineOperations = Get-Section $baseline 'operations'
$baselineIds = @(Get-Array $baselineOperations 'ids')
$baselinePure = @(Get-Array $baselineOperations 'pure')
$baselineMutations = @(Get-Array $baselineOperations 'mutations')
$baselineRecovery = @(Get-Array $baselineOperations 'read_only_recovery')
if ((Get-Int $baselineOperations 'count') -ne 12 -or -not (Same-Sequence $baselineIds @($operationMap.Keys)) -or -not (Same-Sequence $baselinePure $pureIds.ToArray()) -or -not (Same-Sequence $baselineMutations $mutationIds.ToArray()) -or -not (Same-Sequence $baselineRecovery $recoveryIds.ToArray()) -or (Get-String $baselineOperations 'unknown_operation') -cne 'reject') {
    Fail 'Baseline operation closure mismatch.'
}
$baselineRecoverySection = Get-Section $baseline 'recovery'
if (-not (Same-Sequence @(Get-Array $baselineRecoverySection 'dispositions') $recoveryDispositions)) {
    Fail 'Baseline recovery disposition mismatch.'
}
foreach ($unsafe in @('blind_retry_after_possible_write', 'unknown_outcome_may_be_relabelled_success', 'recovery_may_mutate')) {
    if (Get-Bool $baselineRecoverySection $unsafe) { Fail "Unsafe baseline recovery flag: $unsafe" }
}

$p00Context = Get-Section $baseline 'p00_context'
if ((Get-Int $p00Context 'ordinary_source_file_ceiling') -ne (Get-Int $contextDraftManifest 'ordinary_static_source_file_ceiling') -or (Get-Int $p00Context 'search_contracts_exact_pack_source_file_ceiling') -ne (Get-Int $contextDraftManifest 'p00_exact_contract_pack_source_file_ceiling') -or -not (Same-Sequence @(Get-Array $p00Context 'exception_packages') @(Get-Array $contextDraftManifest 'p00_exact_contract_pack_exception_packages')) -or (Get-Int $p00Context 'max_registry_fragments') -ne (Get-Int $contextDraftManifest 'max_registry_fragments_per_context') -or (Get-Int $p00Context 'max_accepted_handoff_slots') -ne (Get-Int $contextDraftManifest 'max_accepted_handoff_slots_per_context') -or (Get-Int $p00Context 'writer_visible_artifacts') -ne 1) {
    Fail 'Baseline P00 context bounds differ from the draft manifest.'
}

$zero = Get-Section $baseline 'zero_state'
foreach ($field in @('materialized_contexts', 'issued_tickets', 'active_writer_leases', 'lease_acknowledgements', 'submissions', 'accepted_reviews', 'accepted_package_handoffs', 'wave_receipts')) {
    if ((Get-Int $zero $field) -ne 0) { Fail "Baseline zero state is nonzero: $field" }
}
if ((Get-String $zero 'active_stage') -cne (Get-String $launch 'active_stage') -or (Get-Int $zero 'active_wave') -ne (Get-Int $launch 'active_wave') -or -not (Same-Sequence @(Get-Array $zero 'authorized_packages') @(Get-Array $launch 'authorized_packages')) -or -not (Same-Sequence @(Get-Array $zero 'conditional_packages') @(Get-Array $launch 'conditional_packages'))) {
    Fail 'Baseline zero state differs from launch state.'
}

if ((Get-Int $fixtures 'schema_version') -ne 1 -or (Get-String $fixtures 'status') -cne 'SYNTHETIC_DESCRIPTORS_NOT_MATERIALIZED' -or (Get-String $fixtures 'owner') -cne 'integration-owner' -or (Get-Int $fixtures 'fixture_count') -ne 52) {
    Fail 'Fixture registry identity/count mismatch.'
}
foreach ($unsafe in @('real_control_record_instances', 'real_actor_assignments', 'source_or_secret_content', 'absolute_local_paths', 'mutable_git_refs')) {
    if (Get-Bool $fixtures $unsafe) { Fail "Fixture registry permits unsafe content: $unsafe" }
}
$fixtureKinds = @(Get-Array $fixtures 'fixture_kinds')
if (-not (Same-Sequence $fixtureKinds @('VALID_BASELINE', 'INVALID_INPUT', 'FAULT_POINT', 'RECOVERY_VIEW', 'POLICY_VIEW'))) {
    Fail 'Fixture kind registry mismatch.'
}
$fixtureBlocks = [regex]::Split($fixtures, '(?m)^\[\[fixture\]\]\s*$')
$fixtureMap = [ordered]@{}
for ($i = 1; $i -lt $fixtureBlocks.Count; $i++) {
    $block = $fixtureBlocks[$i]
    $id = Get-String $block 'id'
    if ($fixtureMap.Contains($id)) { Fail "Duplicate fixture: $id"; continue }
    $fixtureMap[$id] = $block
    if ($id -notmatch '^F\d{3}_[a-z0-9_]+$') { Fail "Invalid fixture ID: $id" }
    if ($fixtureKinds -notcontains (Get-String $block 'kind')) { Fail "$id has unknown fixture kind." }
    $operation = Get-String $block 'operation'
    if (-not $operationMap.Contains($operation)) { Fail "$id references unknown operation $operation." }
    if ([string]::IsNullOrWhiteSpace((Get-String $block 'base')) -or [string]::IsNullOrWhiteSpace((Get-String $block 'mutation'))) { Fail "$id lacks fixture description." }
}
if ($fixtureMap.Count -ne 52) { Fail "Parsed fixture count $($fixtureMap.Count) != 52." }
$fixtureNumbers = @($fixtureMap.Keys | ForEach-Object { [int]([regex]::Match($_, '^F(\d{3})_').Groups[1].Value) })
if (-not (Same-Set $fixtureNumbers @(1..52))) { Fail 'Fixture IDs must cover F001 through F052 exactly.' }

if ((Get-Int $probes 'schema_version') -ne 1 -or (Get-String $probes 'status') -cne 'NOT_EXECUTED' -or (Get-String $probes 'owner') -cne 'integration-owner' -or (Get-Int $probes 'probe_count') -ne 64) {
    Fail 'Probe registry identity/count mismatch.'
}
if (-not (Same-Sequence @(Get-Array $probes 'result_values') @('PASS', 'FAIL', 'UNAVAILABLE')) -or -not (Same-Sequence @(Get-Array $probes 'expected_classes') @('SUCCESS', 'FAILURE', 'RECOVERY', 'POLICY')) -or -not (Get-Bool $probes 'all_mandatory_must_pass') -or (Get-String $probes 'prose_only_evidence') -cne 'reject') {
    Fail 'Probe registry evidence/value closure mismatch.'
}
$probeBlocks = [regex]::Split($probes, '(?m)^\[\[probe\]\]\s*$')
$probeMap = [ordered]@{}
$usedFixtures = [System.Collections.Generic.HashSet[string]]::new([StringComparer]::Ordinal)
$usedOperations = [System.Collections.Generic.HashSet[string]]::new([StringComparer]::Ordinal)
$probeFailures = [System.Collections.Generic.HashSet[string]]::new([StringComparer]::Ordinal)
$probeRecovery = [System.Collections.Generic.HashSet[string]]::new([StringComparer]::Ordinal)
$probeClassCounts = @{ SUCCESS = 0; FAILURE = 0; RECOVERY = 0; POLICY = 0 }
for ($i = 1; $i -lt $probeBlocks.Count; $i++) {
    $block = $probeBlocks[$i]
    $id = Get-String $block 'id'
    if ($probeMap.Contains($id)) { Fail "Duplicate probe: $id"; continue }
    $probeMap[$id] = $block
    if ($id -notmatch '^TI-\d{3}-[a-z0-9-]+$') { Fail "Invalid probe ID: $id" }
    if ((Get-String $block 'owner') -cne 'integration-owner' -or -not (Get-Bool $block 'mandatory')) { Fail "$id owner/mandatory mismatch." }
    $operation = Get-String $block 'operation'
    $fixture = Get-String $block 'fixture'
    if (-not $operationMap.Contains($operation)) { Fail "$id references unknown operation $operation." }
    if (-not $fixtureMap.Contains($fixture)) { Fail "$id references unknown fixture $fixture." }
    [void]$usedOperations.Add($operation)
    [void]$usedFixtures.Add($fixture)
    if ([string]::IsNullOrWhiteSpace((Get-String $block 'variant')) -or [string]::IsNullOrWhiteSpace((Get-String $block 'purpose'))) { Fail "$id lacks variant/purpose." }
    $expectedClass = Get-String $block 'expected_class'
    $expectedValue = Get-String $block 'expected_value'
    if (-not $probeClassCounts.ContainsKey($expectedClass)) { Fail "$id has unknown expected class $expectedClass." }
    else { $probeClassCounts[$expectedClass]++ }
    switch ($expectedClass) {
        'SUCCESS' {
            if ($expectedValue -notin @('SUCCESS', 'IDEMPOTENT_ORIGINAL_RESULT')) { Fail "$id has invalid success value $expectedValue." }
        }
        'FAILURE' {
            if ($failureCodes -notcontains $expectedValue) { Fail "$id has unknown failure value $expectedValue." }
            [void]$probeFailures.Add($expectedValue)
        }
        'RECOVERY' {
            if ($recoveryDispositions -notcontains $expectedValue) { Fail "$id has unknown recovery value $expectedValue." }
            [void]$probeRecovery.Add($expectedValue)
        }
        'POLICY' {
            if ($expectedValue -cne 'NO_AUTHORITY_CHANGE') { Fail "$id has invalid policy value $expectedValue." }
        }
    }
    if ((Get-String $block 'result') -cne 'UNAVAILABLE' -or (Get-String $block 'raw_output_ref') -ne '' -or (Get-String $block 'reviewer_receipt_ref') -ne '') {
        Fail "$id contains premature execution evidence."
    }
}
if ($probeMap.Count -ne 64) { Fail "Parsed probe count $($probeMap.Count) != 64." }
$probeNumbers = @($probeMap.Keys | ForEach-Object { [int]([regex]::Match($_, '^TI-(\d{3})-').Groups[1].Value) })
if (-not (Same-Set $probeNumbers @(1..64))) { Fail 'Probe IDs must cover TI-001 through TI-064 exactly.' }
if (-not (Same-Set $usedFixtures.ToArray() @($fixtureMap.Keys))) { Fail 'Every synthetic fixture must be referenced by at least one probe.' }
if (-not (Same-Set $usedOperations.ToArray() @($operationMap.Keys))) { Fail 'Every operation must be covered by at least one probe.' }
if (-not (Same-Set $probeFailures.ToArray() $failureCodes)) { Fail 'Negative probes do not cover exactly all ClosedReasonCode values.' }
if (-not (Same-Set $probeRecovery.ToArray() $recoveryDispositions)) { Fail 'Recovery probes do not cover every recovery disposition.' }
if ($probeClassCounts['RECOVERY'] -ne 4 -or $probeClassCounts['POLICY'] -ne 1) { Fail 'Probe recovery/policy class counts mismatch.' }

Require-Tokens $qualificationPath $qualification @(
    'QTI-0 — structural closure',
    'QTI-1 — pure and canonical behavior',
    'QTI-2 — append-only mutation and recovery',
    'QTI-3 — authority and noninterference',
    'all 64 mandatory probes = PASS',
    'executed probes:              0',
    'UNAVAILABLE:                 64'
)

if ((Get-Int $controlSchema 'operation_registry_schema_version') -ne 1 -or (Get-String $controlSchema 'operation_registry') -cne $operationsPath -or (Get-String $controlSchema 'qualification_manifest') -cne $manifestPath) {
    Fail 'Control-plane schema does not bind the operation/qualification registries.'
}
if ((Get-Int $orchestration 'schema_version') -ne 6 -or (Get-String $orchestration 'control_plane_operation_registry') -cne $operationsPath -or (Get-String $orchestration 'ticket_issuance_qualification_manifest') -cne $manifestPath -or (Get-String $orchestration 'ticket_issuance_conformance_validator') -cne 'tools/validate-ticket-issuance-conformance.ps1' -or (Get-String $orchestration 'ticket_issuance_conformance_workflow') -cne $workflowPath) {
    Fail 'Orchestration does not bind ticket issuance conformance v1.'
}
if ((Get-Int $launch 'orchestration_registry_schema_version') -ne 6 -or (Get-String $launch 'orchestration_registry_path') -cne $orchestrationPath) {
    Fail 'Launch state does not pin orchestration schema v6.'
}

$workflowDirectory = Join-Path $Root '.github/workflows'
$workflowFiles = @(
    Get-ChildItem $workflowDirectory -File |
        Where-Object { $_.Extension -in @('.yml', '.yaml') } |
        Sort-Object Name
)
foreach ($file in $workflowFiles) {
    Assert-WorkflowPolicy (".github/workflows/" + $file.Name) ([IO.File]::ReadAllText($file.FullName))
}
Require-Tokens $workflowPath $workflow @(
    'actions/checkout@3d3c42e5aac5ba805825da76410c181273ba90b1',
    'runs-on: windows-latest',
    'validate-swarm.ps1',
    'validate-p00-ticket-drafts.ps1',
    'validate-ticket-issuance-contracts.ps1',
    'validate-ticket-issuance-conformance.ps1'
)

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
    Assert-EmptyControlDirectory $directory
}

$result = [ordered]@{
    ok = ($errors.Count -eq 0)
    operations = $operationMap.Count
    fixtures = $fixtureMap.Count
    probes = $probeMap.Count
    failures_covered = $probeFailures.Count
    recovery_dispositions_covered = $probeRecovery.Count
    probe_class_counts = $probeClassCounts
    workflows = $workflowFiles.Count
    executed_probes = 0
    issued_records = 0
    errors = @($errors)
}

if ($Json) {
    $result | ConvertTo-Json -Depth 8
}
else {
    Write-Host "Ticket issuance conformance: operations=$($result.operations) fixtures=$($result.fixtures) probes=$($result.probes) failures=$($result.failures_covered) recovery=$($result.recovery_dispositions_covered) executed=0"
    foreach ($error in $errors) {
        Write-Host "ERROR: $error" -ForegroundColor Red
    }
    if ($result.ok) { Write-Host 'PASS' -ForegroundColor Green }
}

if (-not $result.ok) { exit 1 }
