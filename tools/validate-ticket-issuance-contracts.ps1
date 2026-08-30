[CmdletBinding()]
param(
    [string]$Root = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path,
    [switch]$Json
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
$errors = [System.Collections.Generic.List[string]]::new()
function Fail([string]$Message) { $script:errors.Add($Message) }
function Read-File([string]$Path) {
    $full = Join-Path $Root $Path
    if (-not (Test-Path $full -PathType Leaf)) { Fail "Missing file: $Path"; return '' }
    [IO.File]::ReadAllText($full)
}
function Value([string]$Text, [string]$Key, [bool]$Required = $true) {
    $m = [regex]::Match($Text, ('(?m)^{0}\s*=\s*"([^"]*)"\s*$' -f [regex]::Escape($Key)))
    if (-not $m.Success) { if ($Required) { Fail "Missing string: $Key" }; return '' }
    $m.Groups[1].Value
}
function Number([string]$Text, [string]$Key) {
    $m = [regex]::Match($Text, ('(?m)^{0}\s*=\s*(-?\d+)\s*$' -f [regex]::Escape($Key)))
    if (-not $m.Success) { Fail "Missing integer: $Key"; return [int64]0 }
    [int64]$m.Groups[1].Value
}
function Flag([string]$Text, [string]$Key) {
    $m = [regex]::Match($Text, ('(?m)^{0}\s*=\s*(true|false)\s*$' -f [regex]::Escape($Key)))
    if (-not $m.Success) { Fail "Missing boolean: $Key"; return $false }
    $m.Groups[1].Value -eq 'true'
}
function Array([string]$Text, [string]$Key) {
    $m = [regex]::Match($Text, ('(?ms)^{0}\s*=\s*\[(.*?)\]' -f [regex]::Escape($Key)))
    if (-not $m.Success) { return @() }
    @([regex]::Matches($m.Groups[1].Value, '"([^"\r\n]+)"') | ForEach-Object { $_.Groups[1].Value })
}
function Same([object[]]$A, [object[]]$B) {
    $x = @($A | ForEach-Object { [string]$_ } | Sort-Object -Unique)
    $y = @($B | ForEach-Object { [string]$_ } | Sort-Object -Unique)
    if ($x.Count -ne $y.Count) { return $false }
    for ($i = 0; $i -lt $x.Count; $i++) { if ($x[$i] -cne $y[$i]) { return $false } }
    $true
}
function Require([string]$Path, [string]$Text, [string[]]$Tokens) {
    foreach ($token in $Tokens) {
        if ($Text.IndexOf($token, [StringComparison]::Ordinal) -lt 0) { Fail "$Path lacks: $token" }
    }
}
function Empty-ControlDir([string]$Path) {
    $full = Join-Path $Root $Path
    if (-not (Test-Path $full -PathType Container)) { Fail "Missing directory: $Path"; return }
    foreach ($file in @(Get-ChildItem $full -Recurse -File)) {
        if ($file.Name -notin @('README.md', '.gitkeep')) { Fail "Premature control record: $($file.FullName.Substring($Root.Length + 1))" }
    }
}

$controlPath = 'swarm/control-plane-schema.toml'
$typesPath = 'swarm/schemas/types-v1.toml'
$orchestrationPath = 'swarm/orchestration.toml'
$workflowPath = '.github/workflows/ticket-issuance-contracts.yml'
$control = Read-File $controlPath
$types = Read-File $typesPath
$orchestration = Read-File $orchestrationPath
$workflow = Read-File $workflowPath

$schemas = [ordered]@{
    context_manifest_v1 = @('swarm/schemas/context-manifest-v1.toml', 'swarm/context-manifests/<package>/<context_record_sha256>.toml')
    assignment_ticket_v1 = @('swarm/schemas/assignment-ticket-v1.toml', 'swarm/tickets/<package>/<ticket_id>.toml')
    writer_lease_v1 = @('swarm/schemas/writer-lease-v1.toml', 'swarm/leases/<package>/<lease_id>.toml')
    lease_event_v1 = @('swarm/schemas/lease-event-v1.toml', 'swarm/leases/<package>/events/<event_id>.toml')
    package_submission_v1 = @('swarm/schemas/package-submission-v1.toml', 'swarm/submissions/<package>/<submission_id>.toml')
    independent_review_v1 = @('swarm/schemas/independent-review-v1.toml', 'swarm/reviews/<package>/<review_id>.toml')
    package_handoff_v1 = @('swarm/schemas/package-handoff-v1.toml', 'swarm/handoffs/<package>/<handoff_id>.toml')
    supersession_receipt_v1 = @('swarm/schemas/supersession-receipt-v1.toml', 'swarm/supersessions/<record_kind>/<receipt_id>.toml')
}

if ((Number $types 'schema_version') -ne 1 -or (Value $types 'registry_kind') -cne 'control_plane_types_v1') { Fail 'Invalid type registry identity.' }
if ((Value $types 'unknown_types') -cne 'reject' -or (Value $types 'array_path_kind_semantics') -cne 'element_type') { Fail 'Type registry does not fail closed.' }
foreach ($unsafe in @('implicit_string_coercion_allowed', 'implicit_null_allowed', 'implicit_map_order_allowed')) { if (Flag $types $unsafe) { Fail "Unsafe type flag: $unsafe" } }
$builtins = @(Array $types 'built_in_kinds')
if (-not (Same $builtins @('bool','u16','u32','u64'))) { Fail 'Built-in kind set mismatch.' }

$typeMap = @{}
$typeRep = @{}
$typeBlocks = [regex]::Split($types, '(?m)^\[\[type\]\]\s*$')
for ($i = 1; $i -lt $typeBlocks.Count; $i++) {
    $name = Value $typeBlocks[$i] 'name'
    $rep = Value $typeBlocks[$i] 'representation'
    if ($typeMap.ContainsKey($name)) { Fail "Duplicate type: $name" } else { $typeMap[$name] = $typeBlocks[$i]; $typeRep[$name] = $rep }
}
foreach ($name in @($typeMap.Keys)) {
    $rep = [string]$typeRep[$name]
    if ($rep -in @('string','tagged_string','ordered_record')) { continue }
    $g = [regex]::Match($rep, '^(?:OptionalV1|list)<([A-Za-z0-9_]+)>$')
    $target = if ($g.Success) { $g.Groups[1].Value } else { $rep }
    if (-not $typeMap.ContainsKey($target) -and $builtins -notcontains $target) { Fail "Unresolved type alias: $name -> $target" }
}
if ([string]$typeRep['OrderedImmutableRecordRef'] -match '^list<') { Fail 'OrderedImmutableRecordRef must be an element type.' }
if ([string]$typeRep['OrderedDigestSet'] -notmatch '^list<') { Fail 'OrderedDigestSet must be a collection type.' }

if ((Number $control 'schema_version') -ne 2 -or (Value $control 'type_registry') -cne $typesPath) { Fail 'Invalid control-plane registry identity.' }
$required = @(Array $control 'required_schema_files')
$expectedFiles = @($typesPath) + @($schemas.Values | ForEach-Object { $_[0] })
if (-not (Same $required $expectedFiles)) { Fail 'required_schema_files is not closed.' }

$recordMap = @{}
$layoutMap = @{}
$recordBlocks = [regex]::Split($control, '(?m)^\[\[record\]\]\s*$')
for ($i = 1; $i -lt $recordBlocks.Count; $i++) {
    $kind = Value $recordBlocks[$i] 'kind'
    if ($recordMap.ContainsKey($kind)) { Fail "Duplicate record kind: $kind"; continue }
    $recordMap[$kind] = Value $recordBlocks[$i] 'path'
    $layoutMap[$kind] = Value $recordBlocks[$i] 'canonical_layout'
}
if (-not (Same @($recordMap.Keys) @($schemas.Keys))) { Fail 'Control record set mismatch.' }
$closedKinds = @(Array ([string]$typeMap['ClosedControlRecordKind']) 'allowed')
if (-not (Same $closedKinds @($schemas.Keys))) { Fail 'ClosedControlRecordKind mismatch.' }

$totalFields = 0
foreach ($entry in $schemas.GetEnumerator()) {
    $kind = $entry.Key; $path = $entry.Value[0]; $layout = $entry.Value[1]; $schema = Read-File $path
    if ((Number $schema 'schema_version') -ne 1 -or (Value $schema 'record_kind') -cne $kind) { Fail "$path identity mismatch." }
    if ((Value $schema 'status') -cne 'SCHEMA_ONLY_NOT_AN_INSTANCE' -or -not (Flag $schema 'immutable') -or (Value $schema 'unknown_fields') -cne 'reject') { Fail "$path is not an immutable fail-closed schema." }
    if ((Value $schema 'record_layout') -cne $layout -or [string]$recordMap[$kind] -cne $path -or [string]$layoutMap[$kind] -cne $layout) { Fail "$path layout/registry mismatch." }
    if ($schema -match '(?m)^(>:source_content(?:_in_record)?|secret_content(?:_in_record)?|absolute_local_paths_allowed)\s*=\s*true\s*$') { Fail "$path permits forbidden content." }
    $fields = [regex]::Split($schema, '(?m)^\[\[field\]\]\s*$')
    $seenPath = [System.Collections.Generic.HashSet[string]]::new([StringComparer]::Ordinal)
    $seenOrder = [System.Collections.Generic.HashSet[int]]::new()
    $signatureCount = 0
    for ($i = 1; $i -lt $fields.Count; $i++) {
        $p = Value $fields[$i] 'path'; $k = Value $fields[$i] 'kind'; $n = [int](Number $fields[$i] 'canonical_order'); $rules = @(Array $fields[$i] 'rules')
        $totalFields++
        if (-not $seenPath.Add($p) -or -not $seenOrder.Add($n)) { Fail "$path duplicates field path/order at $p." }
        if (-not (Flag $fields[$i] 'required') -or $rules.Count -eq 0) { Fail "$path field $p lacks required wrapper/rules." }
        if ($builtins -notcontains $k -and -not $typeMap.ContainsKey($k)) { Fail "$path field $p has unknown kind $k." }
        if ($p.EndsWith('[]') -and $typeMap.ContainsKey($k) -and [string]$typeRep[$k] -match '^list<') { Fail "$path field $p is list-of-list." }
        if ($k -ceq 'ClosedEnum' -and ($rules -join ' ') -notmatch '(?:equals_|one_of_|PASS_)') { Fail "$path field $p has an open enum." }
        if ($p -ceq 'signature.record_sha256') { $signatureCount++; if (($rules -join ' ') -notmatch '(?:exact_record_digest_rule|signed_payload_sha256_before_signature_table)') { Fail "$path has wrong embedded digest semantics." } }
    }
    if (-not (Same @($seenOrder) @(1..($fields.Count - 1)))) { Fail "$path canonical orders are not contiguous." }
    if ($signatureCount -ne 1) { Fail "$path must define one signature.record_sha256." }
}

if ((Number $orchestration 'schema_version') -ne 4) { Fail 'Orchestration schema_version must be 4.' }
foreach ($pair in @(
    @('control_plane_schema_registry',$controlPath), @('control_plane_type_registry',$typesPath),
    @('control_plane_validator','tools/validate-ticket-issuance-contracts.ps1'), @('control_plane_manual_workflow',$workflowPath)
)) { if ((Value $orchestration $pair[0]) -cne $pair[1]) { Fail "Orchestration path mismatch: $($pair[0])" } }
if ((Value $orchestration 'accepted_handoff_layout') -cne 'swarm/handoffs/<package>/<handoff-id>.toml') { Fail 'Handoff path still uses API identity.' }
Require 'swarm/RECEIPT_CANONICALIZATION.md' (Read-File 'swarm/RECEIPT_CANONICALIZATION.md') @('signed_payload_sha256','exact_record_file_sha256','fixed-point self-hash')
Require 'docs/handoff/TICKET_ISSUANCE_OPERATIONS.md' (Read-File 'docs/handoff/TICKET_ISSUANCE_OPERATIONS.md') @('CONTROL_OPERATION_CONFLICT','recover_control_operation')
if ($workflow -match '(?m)^\s*(pull_request|push|schedule|workflow_run|repository_dispatch|workflow_call):') { Fail 'Workflow is not manual-only.' }
Require $workflowPath $workflow @('workflow_dispatch:','contents: read','persist-credentials: false','validate-p00-ticket-drafts.ps1','validate-ticket-issuance-contracts.ps1')
foreach ($dir in @('swarm/tickets','swarm/context-manifests','swarm/leases','swarm/submissions','swarm/reviews','swarm/handoffs','swarm/supersessions','swarm/wave-receipts')) { Empty-ControlDir $dir }

$result = [ordered]@{ ok = ($errors.Count -eq 0); types = $typeMap.Count; records = $schemas.Count; fields = $totalFields; issued_records = 0; errors = @($errors) }
if ($Json) { $result | ConvertTo-Json -Depth 6 } else { Write-Host "Ticket issuance schemas: types=$($result.types) records=$($result.records) fields=$($result.fields)"; foreach ($e in $errors) { Write-Host "ERROR: $e" -ForegroundColor Red }; if ($result.ok) { Write-Host 'PASS' -ForegroundColor Green } }
if (-not $result.ok) { exit 1 }
