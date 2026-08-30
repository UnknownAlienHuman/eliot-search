[CmdletBinding()]
param(
    [string]$Root = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path,
    [switch]$Json,
    [string]$Python = 'python'
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

$errors = [System.Collections.Generic.List[string]]::new()
function Fail([string]$Message) { $script:errors.Add($Message) }
function Read-Required([string]$RelativePath) {
    $path = Join-Path $Root $RelativePath
    if (-not (Test-Path $path -PathType Leaf)) {
        Fail "Missing required file: $RelativePath"
        return ''
    }
    [IO.File]::ReadAllText($path)
}
function String-Value([string]$Text, [string]$Key, [bool]$Required = $true) {
    $match = [regex]::Match($Text, ('(?m)^{0}\s*=\s*"([^"]*)"\s*$' -f [regex]::Escape($Key)))
    if (-not $match.Success) {
        if ($Required) { Fail "Missing string value: $Key" }
        return ''
    }
    $match.Groups[1].Value
}
function Int-Value([string]$Text, [string]$Key) {
    $match = [regex]::Match($Text, ('(?m)^{0}\s*=\s*(\d+)\s*$' -f [regex]::Escape($Key)))
    if (-not $match.Success) { Fail "Missing integer value: $Key"; return [int64]0 }
    [int64]$match.Groups[1].Value
}
function Bool-Value([string]$Text, [string]$Key) {
    $match = [regex]::Match($Text, ('(?m)^{0}\s*=\s*(true|false)\s*$' -f [regex]::Escape($Key)))
    if (-not $match.Success) { Fail "Missing boolean value: $Key"; return $false }
    $match.Groups[1].Value -eq 'true'
}
function String-Array([string]$Text, [string]$Key) {
    $match = [regex]::Match($Text, ('(?ms)^{0}\s*=\s*\[(.*?)\]' -f [regex]::Escape($Key)))
    if (-not $match.Success) { return @() }
    @([regex]::Matches($match.Groups[1].Value, '"([^"\r\n]+)"') | ForEach-Object { $_.Groups[1].Value })
}
function Require-Tokens([string]$Path, [string]$Text, [string[]]$Tokens) {
    foreach ($token in $Tokens) {
        if ($Text.IndexOf($token, [StringComparison]::Ordinal) -lt 0) {
            Fail "$Path lacks required token: $token"
        }
    }
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
function Assert-WorkflowPolicy([IO.FileInfo]$File) {
    $body = [IO.File]::ReadAllText($File.FullName)
    $relative = $File.FullName.Substring($Root.Length + 1).Replace('\\', '/')
    if (-not [regex]::IsMatch($body, '(?m)^\s{2}workflow_dispatch:\s*$')) {
        Fail "$relative lacks workflow_dispatch."
    }
    $forbidden = '(?m)^\s{0,6}(push|pull_request|pull_request_target|merge_group|schedule|workflow_run|repository_dispatch|workflow_call|release|issues|issue_comment|discussion|discussion_comment|create|delete|branch_protection_rule|check_run|check_suite|deployment|deployment_status|fork|gollum|label|milestone|page_build|project|project_card|project_column|public|registry_package|status|watch):\s*$'
    if ([regex]::IsMatch($body, $forbidden)) { Fail "$relative contains an automatic trigger." }
    if (-not [regex]::IsMatch($body, '(?m)^\s{2}contents:\s*read\s*$')) {
        Fail "$relative is not contents: read."
    }
    if ($body.IndexOf('persist-credentials: false', [StringComparison]::Ordinal) -lt 0) {
        Fail "$relative persists checkout credentials."
    }
}
function Assert-ZeroState([string]$RelativeRoot) {
    $path = Join-Path $Root $RelativeRoot
    if (-not (Test-Path $path -PathType Container)) {
        Fail "Missing protected root: $RelativeRoot"
        return
    }
    foreach ($file in @(Get-ChildItem $path -Recurse -File)) {
        if ($file.Name -notin @('README.md', '.gitkeep')) {
            Fail "Premature control record: $($file.FullName.Substring($Root.Length + 1))"
        }
    }
}

$registryPath = 'swarm/ticket-issuance-planner.toml'
$schemaPath = 'swarm/ticket-issuance-plan-schema.toml'
$digestPath = 'swarm/ticket-issuance-plan-digest-v1.toml'
$contractPath = 'docs/handoff/TICKET_ISSUANCE_PLANNER.md'
$digestContractPath = 'docs/handoff/TICKET_ISSUANCE_PLANNER_DIGEST_RULE.md'
$plannerPath = 'tools/plan-ticket-issuance.py'
$wrapperPath = 'tools/plan-ticket-issuance.ps1'
$casesPath = 'qualification/ticket-issuance/cases.toml'
$runnerPath = 'qualification/ticket-issuance/run_planner_tests.py'
$testsPath = 'qualification/ticket-issuance/test_plan_ticket_issuance.py'
$workflowPath = '.github/workflows/ticket-issuance-plan-qualified.yml'

$registry = Read-Required $registryPath
$schema = Read-Required $schemaPath
$digest = Read-Required $digestPath
$contract = Read-Required $contractPath
$digestContract = Read-Required $digestContractPath
$planner = Read-Required $plannerPath
$wrapper = Read-Required $wrapperPath
$cases = Read-Required $casesPath
$runner = Read-Required $runnerPath
$tests = Read-Required $testsPath
$workflow = Read-Required $workflowPath

if ((Int-Value $registry 'schema_version') -ne 1 -or (String-Value $registry 'component') -cne 'ticket_issuance_planner_v1') {
    Fail 'Invalid planner registry identity.'
}
if ((String-Value $registry 'status') -cne 'ADVISORY_DRY_RUN_ONLY') {
    Fail 'Planner registry status is not advisory dry-run only.'
}
$registeredPaths = [ordered]@{
    contract = $contractPath
    digest_contract = $digestContractPath
    plan_schema = $schemaPath
    digest_profile = $digestPath
    implementation = $plannerPath
    powershell_wrapper = $wrapperPath
    structural_validator = 'tools/validate-ticket-issuance-plan.ps1'
    qualification_cases = $casesPath
    qualification_runner = $runnerPath
    qualification_tests = $testsPath
    manual_workflow = $workflowPath
}
foreach ($entry in $registeredPaths.GetEnumerator()) {
    if ((String-Value $registry $entry.Key) -cne $entry.Value) {
        Fail "Planner registry path mismatch: $($entry.Key)"
    }
}
foreach ($flag in @(
    'output_is_control_record',
    'output_is_claimable',
    'output_is_evidence_receipt',
    'may_materialize_context',
    'may_issue_ticket',
    'may_issue_or_acknowledge_lease',
    'may_authorize_implementation',
    'may_record_submission_or_review',
    'may_publish_package_handoff',
    'may_accept_gate_or_wave',
    'may_advance_launch_state',
    'repository_mutations',
    'protected_root_writes',
    'working_tree_source_of_truth'
)) {
    if (Bool-Value $registry $flag) { Fail "Unsafe planner registry flag enabled: $flag" }
}
foreach ($flag in @('deterministic', 'exact_base_commit_validation_supported')) {
    if (-not (Bool-Value $registry $flag)) { Fail "Required planner registry flag disabled: $flag" }
}

if ((Int-Value $schema 'schema_version') -ne 1 -or (String-Value $schema 'record_kind') -cne 'ticket_issuance_plan_v1') {
    Fail 'Invalid advisory plan schema identity.'
}
if ((String-Value $schema 'status') -cne 'ADVISORY_NON_AUTHORITATIVE') {
    Fail 'Plan schema is not non-authoritative.'
}
foreach ($flag in @(
    'output_is_control_record',
    'output_is_evidence_receipt',
    'output_is_claimable',
    'output_may_be_written_under_protected_roots'
)) {
    if (Bool-Value $schema $flag) { Fail "Unsafe plan schema flag enabled: $flag" }
}
$expectedDecisions = @(
    'READY_FOR_CONTEXT_MATERIALIZATION_PREVIEW',
    'BLOCKED_MISSING_SELECTION',
    'BLOCKED_PREREQUISITE',
    'BLOCKED_CONFLICT',
    'INVALID_REPOSITORY_STATE'
)
if (-not (Same-Set @(String-Array $schema 'closed_decisions') $expectedDecisions)) {
    Fail 'Closed planner decision set mismatch.'
}
$schemaReasons = @(String-Array $schema 'closed_reason_codes')
if ($schemaReasons.Count -ne 24 -or $schemaReasons.Count -ne @($schemaReasons | Sort-Object -Unique).Count) {
    Fail 'Planner reason registry must contain 24 unique values.'
}
foreach ($required in @(
    'PARTIAL_ISSUANCE_SELECTION',
    'WRITER_REVIEWER_CONFLICT',
    'HANDOFF_SLOT_UNSATISFIED',
    'PROTECTED_ROOT_NOT_ZERO_STATE',
    'WORKFLOW_POLICY_VIOLATION',
    'OUTPUT_PATH_PROTECTED'
)) {
    if ($schemaReasons -notcontains $required) { Fail "Planner reason registry omits: $required" }
}
foreach ($flag in @(
    'mutations_must_be_empty',
    'authorizes_ticket_issuance_must_be_false',
    'creates_writer_lease_must_be_false',
    'authorizes_implementation_must_be_false',
    'publishes_package_handoff_must_be_false',
    'advances_launch_state_must_be_false'
)) {
    if (-not (Bool-Value $schema $flag)) { Fail "Plan schema non-authority invariant disabled: $flag" }
}

if ((Int-Value $digest 'schema_version') -ne 1 -or (String-Value $digest 'profile') -cne 'ticket_issuance_plan_digest_v1') {
    Fail 'Invalid planner digest profile identity.'
}
if ((String-Value $digest 'canonical_payload') -cne 'complete_canonical_plan_object_with_plan_sha256_field_omitted') {
    Fail 'Planner digest payload rule is not explicit.'
}
foreach ($flag in @('self_referential_digest_allowed', 'placeholder_replacement_allowed', 'parsed_reserialization_allowed')) {
    if (Bool-Value $digest $flag) { Fail "Unsafe planner digest flag enabled: $flag" }
}
Require-Tokens $digestContractPath $digestContract @(
    'plan_sha256` omitted',
    'fixed-point/self-referential hashing',
    'advisory digest',
    'complete-file SHA-256'
)

if ((Int-Value $cases 'case_count') -ne 18) { Fail 'Planner qualification case_count must be 18.' }
$caseIds = @([regex]::Matches($cases, '(?m)^id\s*=\s*"(PLAN-\d{3})"\s*$') | ForEach-Object { $_.Groups[1].Value })
if ($caseIds.Count -ne 18 -or $caseIds.Count -ne @($caseIds | Sort-Object -Unique).Count) {
    Fail 'Planner qualification IDs must be 18 unique PLAN-NNN values.'
}
foreach ($decision in $expectedDecisions) {
    if ($cases.IndexOf($decision, [StringComparison]::Ordinal) -lt 0) {
        Fail "Qualification corpus omits decision: $decision"
    }
}

Require-Tokens $contractPath $contract @(
    'mutations = []',
    'authorizes_ticket_issuance = false',
    'creates_writer_lease = false',
    'authorizes_implementation = false',
    'publishes_package_handoff = false',
    'advances_launch_state = false',
    'PARTIAL_ISSUANCE_SELECTION',
    'OUTPUT_PATH_PROTECTED'
)
Require-Tokens $plannerPath $planner @(
    'DOMAIN_SEPARATOR = b"eliot-search/ticket-issuance-plan/v1\\0"',
    '"mutations": []',
    '"authorizes_ticket_issuance": False',
    '"creates_writer_lease": False',
    '"authorizes_implementation": False',
    '"publishes_package_handoff": False',
    '"advances_launch_state": False',
    'validate_tagged_commit',
    'validate_zero_state',
    'validate_workflows',
    'validate_output_path',
    'plan_digest'
)
foreach ($forbidden in @(
    'git push',
    'git commit',
    'merge_pull_request',
    'create_pull_request',
    'swarm/tickets/<package>',
    'swarm/leases/<package>'
)) {
    if ($planner.IndexOf($forbidden, [StringComparison]::OrdinalIgnoreCase) -ge 0) {
        Fail "Planner contains forbidden mutation token: $forbidden"
    }
}
Require-Tokens $wrapperPath $wrapper @('--package', '--output', '--require-ready')
Require-Tokens $runnerPath $runner @('sys.modules[name] = module', 'unittest.TextTestRunner')
Require-Tokens $testsPath $tests @(
    'test_complete_valid_selection_is_preview_ready',
    'test_partial_selection_fails_closed',
    'test_conditional_package_without_handoff',
    'test_issued_record_breaks_zero_state',
    'test_current_repository_search_contracts_is_non_authoritative'
)

$workflowFiles = @(
    Get-ChildItem (Join-Path $Root '.github/workflows') -File |
        Where-Object { $_.Extension -in @('.yml', '.yaml') } |
        Sort-Object Name
)
foreach ($file in $workflowFiles) { Assert-WorkflowPolicy $file }
Require-Tokens $workflowPath $workflow @(
    'runs-on: windows-latest',
    'run_planner_tests.py',
    '--package search-contracts',
    'BLOCKED_MISSING_SELECTION',
    'validate-ticket-issuance-contracts.ps1'
)

foreach ($protected in @(
    'swarm/context-manifests',
    'swarm/tickets',
    'swarm/leases',
    'swarm/submissions',
    'swarm/reviews',
    'swarm/handoffs',
    'swarm/supersessions',
    'swarm/wave-receipts'
)) {
    Assert-ZeroState $protected
}

$actualPlan = $null
try {
    $rawPlan = & $Python (Join-Path $Root $plannerPath) --root $Root --package search-contracts --output - 2>&1 | Out-String
    if ($LASTEXITCODE -ne 0) {
        Fail "Actual planner execution failed with exit code $LASTEXITCODE."
    }
    else {
        $actualPlan = $rawPlan | ConvertFrom-Json
        if ($actualPlan.decision -ne 'BLOCKED_MISSING_SELECTION') {
            Fail "Actual repository plan decision is $($actualPlan.decision)."
        }
        if ($actualPlan.mutations.Count -ne 0) { Fail 'Actual plan contains mutations.' }
        foreach ($field in @(
            'authorizes_ticket_issuance',
            'creates_writer_lease',
            'authorizes_implementation',
            'publishes_package_handoff',
            'advances_launch_state'
        )) {
            if ($actualPlan.$field -ne $false) { Fail "Actual plan enabled authority field: $field" }
        }
    }
}
catch {
    Fail "Unable to execute/parse actual planner output: $($_.Exception.Message)"
}

$result = [ordered]@{
    ok = ($errors.Count -eq 0)
    registry_schema = Int-Value $registry 'schema_version'
    plan_schema = Int-Value $schema 'schema_version'
    digest_schema = Int-Value $digest 'schema_version'
    reason_codes = $schemaReasons.Count
    qualification_cases = $caseIds.Count
    workflows = $workflowFiles.Count
    actual_decision = if ($actualPlan) { [string]$actualPlan.decision } else { 'UNAVAILABLE' }
    mutations = 0
    authority = $false
    errors = @($errors)
}

if ($Json) {
    $result | ConvertTo-Json -Depth 8
}
else {
    Write-Host "Ticket issuance planner: reasons=$($result.reason_codes) cases=$($result.qualification_cases) workflows=$($result.workflows) decision=$($result.actual_decision)"
    foreach ($error in $errors) { Write-Host "ERROR: $error" -ForegroundColor Red }
    if ($result.ok) { Write-Host 'PASS' -ForegroundColor Green }
}
if (-not $result.ok) { exit 1 }
