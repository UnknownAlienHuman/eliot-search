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
    $pattern = '(?ms)^{0}[ \t]*=[ \t]*\[(.*?)\][ \t]*\r?$' -f [regex]::Escape($Key)
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

function Assert-EmptyControlDirectory([string]$Path) {
    $full = Join-Path $Root $Path
    if (-not (Test-Path $full -PathType Container)) {
        Fail "Missing directory: $Path"
        return
    }
    foreach ($file in @(Get-ChildItem $full -Recurse -File)) {
        if ($file.Name -notin @('README.md', '.gitkeep')) {
            $relative = $file.FullName.Substring($Root.Length + 1)
            Fail "Premature control record: $relative"
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
    if ([regex]::IsMatch($Text, '(?m)^\s{2}contents:\s*write\s*$')) {
        Fail "$Path grants repository write permission."
    }
    if ($Text.IndexOf('persist-credentials: false', [StringComparison]::Ordinal) -lt 0) {
        Fail "$Path must disable checkout credential persistence."
    }
}

$controlPath = 'swarm/control-plane-schema.toml'
$typesPath = 'swarm/schemas/types-v1.toml'
$schemaReadmePath = 'swarm/schemas/README.md'
$orchestrationPath = 'swarm/orchestration.toml'
$launchPath = 'swarm/launch-state.toml'
$operationsPath = 'docs/handoff/TICKET_ISSUANCE_OPERATIONS.md'
$canonicalizationPath = 'swarm/RECEIPT_CANONICALIZATION.md'
$handoffReadmePath = 'docs/handoff/README.md'
$workflowPath = '.github/workflows/ticket-issuance-contracts.yml'

$control = Read-File $controlPath
$types = Read-File $typesPath
$schemaReadme = Read-File $schemaReadmePath
$orchestration = Read-File $orchestrationPath
$launch = Read-File $launchPath
$operations = Read-File $operationsPath
$canonicalization = Read-File $canonicalizationPath
$handoffReadme = Read-File $handoffReadmePath
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

$expectedFailures = @(
    'DRAFT_NOT_VALID',
    'DRAFT_NOT_NONCLAIMABLE',
    'CONTEXT_DRAFT_INVALID',
    'CONTEXT_SOURCE_MISSING',
    'CONTEXT_SOURCE_NOT_UTF8',
    'CONTEXT_SELECTOR_UNSUPPORTED',
    'CONTEXT_SELECTOR_NOT_UNIQUE',
    'CONTEXT_FORBIDDEN_PATH',
    'CONTEXT_HANDOFF_MISSING',
    'CONTEXT_HANDOFF_MISMATCH',
    'CONTEXT_BUDGET_EXCEEDED',
    'CONTEXT_MATERIALIZATION_CANCELLED',
    'CONTEXT_MATERIALIZATION_OUTCOME_UNKNOWN',
    'TICKET_PREREQUISITE_MISSING',
    'TICKET_WRITER_REVIEWER_CONFLICT',
    'TICKET_ISSUE_OUTCOME_UNKNOWN',
    'TICKET_OPERATION_CONFLICT',
    'PACKAGE_LEASE_CONFLICT',
    'LEASE_ACKNOWLEDGEMENT_MISMATCH',
    'LEASE_REVOKED_OR_SUPERSEDED',
    'SUBMISSION_SCOPE_VIOLATION',
    'SUBMISSION_DIFF_INCOMPLETE',
    'SUBMISSION_EVIDENCE_INCOMPLETE',
    'SUBMISSION_LINE_BUDGET_VIOLATION',
    'REVIEW_NOT_INDEPENDENT',
    'REVIEW_RECALCULATION_MISMATCH',
    'PACKAGE_HANDOFF_NOT_ACCEPTED',
    'CONTROL_RECORD_SCHEMA_MISMATCH',
    'CONTROL_RECORD_DIGEST_MISMATCH',
    'CONTROL_OPERATION_CONFLICT',
    'CONTROL_RECORD_QUARANTINED'
)

$legacyAmbiguousRecordDigestPaths = [System.Collections.Generic.HashSet[string]]::new([StringComparer]::Ordinal)
foreach ($legacyPath in @(
    'draft.exact_sha256',
    'context.manifest_sha256',
    'ticket.sha256',
    'lease.sha256',
    'chain.previous_event_sha256',
    'ticket_lease_context.ticket_sha256',
    'ticket_lease_context.lease_sha256',
    'ticket_lease_context.context_manifest_sha256',
    'submission.sha256',
    'submission_review.submission_sha256',
    'submission_review.review_sha256',
    'supersession.supersedes_handoff_sha256',
    'old_record.sha256',
    'replacement_record.sha256'
)) {
    [void]$legacyAmbiguousRecordDigestPaths.Add($legacyPath)
}

if ((Get-Int $types 'schema_version') -ne 2 -or (Get-String $types 'registry_kind') -cne 'control_plane_types_v1') {
    Fail 'Invalid type registry identity.'
}
if ((Get-String $types 'unknown_types') -cne 'reject' -or (Get-String $types 'array_path_kind_semantics') -cne 'element_type') {
    Fail 'Type registry does not fail closed.'
}
foreach ($unsafe in @('implicit_string_coercion_allowed', 'implicit_null_allowed', 'implicit_map_order_allowed')) {
    if (Get-Bool $types $unsafe) { Fail "Unsafe type flag: $unsafe" }
}

$builtins = @(Get-Array $types 'built_in_kinds')
if (-not (Same-Set $builtins @('bool', 'u16', 'u32', 'u64'))) {
    Fail 'Built-in kind set mismatch.'
}

$typeMap = @{}
$typeRep = @{}
$typeBlocks = [regex]::Split($types, '(?m)^\[\[type\]\]\s*$')
for ($i = 1; $i -lt $typeBlocks.Count; $i++) {
    $name = Get-String $typeBlocks[$i] 'name'
    $representation = Get-String $typeBlocks[$i] 'representation'
    if ($typeMap.ContainsKey($name)) {
        Fail "Duplicate type: $name"
    }
    else {
        $typeMap[$name] = $typeBlocks[$i]
        $typeRep[$name] = $representation
    }
}

$terminalRepresentations = @('string', 'tagged_string', 'ordered_record')
foreach ($name in @($typeMap.Keys)) {
    $seen = [System.Collections.Generic.HashSet[string]]::new([StringComparer]::Ordinal)
    $cursor = $name
    while ($true) {
        if (-not $seen.Add($cursor)) {
            Fail "Type alias cycle: $name"
            break
        }

        $representation = [string]$typeRep[$cursor]
        if ($terminalRepresentations -contains $representation) { break }

        $generic = [regex]::Match($representation, '^(?:OptionalV1|list)<([A-Za-z0-9_]+)>$')
        $target = if ($generic.Success) { $generic.Groups[1].Value } else { $representation }

        if ($builtins -contains $target) { break }
        if (-not $typeMap.ContainsKey($target)) {
            Fail "Unresolved type alias: $cursor -> $target"
            break
        }
        $cursor = $target
    }
}

if ([string]$typeRep['OrderedImmutableRecordRef'] -match '^list<') {
    Fail 'OrderedImmutableRecordRef must be an element type.'
}
if ([string]$typeRep['OrderedDigestSet'] -notmatch '^list<') {
    Fail 'OrderedDigestSet must be a collection type.'
}

$immutableRecordFields = @(Get-Array ([string]$typeMap['ImmutableRecordRef']) 'canonical_fields')
if (-not (Same-Sequence $immutableRecordFields @('repository', 'commit', 'path', 'git_blob_id', 'exact_record_file_sha256', 'record_kind'))) {
    Fail 'ImmutableRecordRef canonical field set is inconsistent with exact-file identity.'
}

$artifactRules = @(Get-Array ([string]$typeMap['ImmutableArtifactRef']) 'rules')
foreach ($rule in @('bytes_is_u64', 'zero_length_allowed_when_exact')) {
    if ($artifactRules -notcontains $rule) { Fail "ImmutableArtifactRef lacks: $rule" }
}

$reasonValues = @(Get-Array ([string]$typeMap['ClosedReasonCode']) 'allowed')
if (-not (Same-Sequence $reasonValues $expectedFailures)) {
    Fail 'ClosedReasonCode does not match the exact operation failure registry.'
}
if (-not (Same-Sequence @(Get-Array ([string]$typeMap['LeaseEventReasonCode']) 'allowed') @('WRITER_ACKNOWLEDGED', 'PACKAGE_SUBMITTED', 'LEASE_REVOKED', 'LEASE_SUPERSEDED'))) {
    Fail 'LeaseEventReasonCode mismatch.'
}
if (-not (Same-Sequence @(Get-Array ([string]$typeMap['SupersessionReasonCode']) 'allowed') @('RECORD_CORRECTION', 'RECORD_REPLACEMENT', 'AUTHORITY_REVOKED', 'CONTRACT_SUPERSEDED', 'EVIDENCE_SUPERSEDED'))) {
    Fail 'SupersessionReasonCode mismatch.'
}
if (-not (Same-Sequence @(Get-Array ([string]$typeMap['ConsumerActionCode']) 'allowed') @('NO_ACTION', 'REVALIDATE_COMPATIBILITY', 'ADOPT_ADDITIVE_SURFACE', 'MIGRATE_BEFORE_STAGE', 'BLOCK_UNTIL_MIGRATED'))) {
    Fail 'ConsumerActionCode mismatch.'
}
if ((@(Get-Array ([string]$typeMap['OrderedConsumerAction']) 'rules')) -notcontains 'action_code_is_ConsumerActionCode') {
    Fail 'OrderedConsumerAction is not bound to ConsumerActionCode.'
}

if ((Get-Int $control 'schema_version') -ne 3 -or (Get-String $control 'type_registry') -cne $typesPath) {
    Fail 'Invalid control-plane registry identity.'
}
if ((Get-Int $control 'type_registry_schema_version') -ne 2) {
    Fail 'Control-plane registry does not pin type registry schema v2.'
}
$reasonTypeBindings = [ordered]@{
    failure_reason_type = 'ClosedReasonCode'
    lease_event_reason_type = 'LeaseEventReasonCode'
    supersession_reason_type = 'SupersessionReasonCode'
    consumer_action_type = 'ConsumerActionCode'
}
foreach ($entry in $reasonTypeBindings.GetEnumerator()) {
    if ((Get-String $control $entry.Key) -cne $entry.Value) {
        Fail "Control-plane reason binding mismatch: $($entry.Key)"
    }
}
if ((Get-String $control 'workflow_policy') -cne 'manual_only') {
    Fail 'Control-plane workflow policy must be manual_only.'
}
foreach ($unsafe in @(
    'in_place_mutation_allowed',
    'source_content_allowed',
    'secret_content_allowed',
    'absolute_local_paths_allowed',
    'dependency_implementation_source_allowed',
    'self_referential_complete_file_digest_allowed'
)) {
    if (Get-Bool $control $unsafe) { Fail "Unsafe control-plane flag: $unsafe" }
}

$requiredSchemaFiles = @(Get-Array $control 'required_schema_files')
$expectedSchemaFiles = @($typesPath) + @($schemas.Values | ForEach-Object { $_[0] })
if (-not (Same-Set $requiredSchemaFiles $expectedSchemaFiles)) {
    Fail 'required_schema_files is not closed.'
}
if ((Get-Int $control 'registered_types') -ne $typeMap.Count) {
    Fail 'Control-plane registered_types count mismatch.'
}

$recordMap = @{}
$layoutMap = @{}
$recordBlocks = [regex]::Split($control, '(?m)^\[\[record\]\]\s*$')
for ($i = 1; $i -lt $recordBlocks.Count; $i++) {
    $kind = Get-String $recordBlocks[$i] 'kind'
    if ($recordMap.ContainsKey($kind)) {
        Fail "Duplicate record kind: $kind"
        continue
    }
    $recordMap[$kind] = Get-String $recordBlocks[$i] 'path'
    $layoutMap[$kind] = Get-String $recordBlocks[$i] 'canonical_layout'
}
if (-not (Same-Set @($recordMap.Keys) @($schemas.Keys))) {
    Fail 'Control record set mismatch.'
}

$closedKinds = @(Get-Array ([string]$typeMap['ClosedControlRecordKind']) 'allowed')
if (-not (Same-Sequence $closedKinds @($schemas.Keys))) {
    Fail 'ClosedControlRecordKind mismatch.'
}

$totalFields = 0
$totalSignatureRefs = 0
$schemaFieldPaths = @{}
$schemaFieldKinds = @{}
foreach ($entry in $schemas.GetEnumerator()) {
    $kind = [string]$entry.Key
    $path = [string]$entry.Value[0]
    $layout = [string]$entry.Value[1]
    $schema = Read-File $path

    if ((Get-Int $schema 'schema_version') -ne 1 -or (Get-String $schema 'record_kind') -cne $kind) {
        Fail "$path identity mismatch."
    }
    if ((Get-String $schema 'status') -cne 'SCHEMA_ONLY_NOT_AN_INSTANCE' -or -not (Get-Bool $schema 'immutable') -or (Get-String $schema 'unknown_fields') -cne 'reject') {
        Fail "$path is not an immutable fail-closed schema."
    }
    if ((Get-String $schema 'canonicalization') -cne 'exact_utf8_lf_sha256') {
        Fail "$path canonicalization mismatch."
    }
    if ((Get-String $schema 'record_layout') -cne $layout -or [string]$recordMap[$kind] -cne $path -or [string]$layoutMap[$kind] -cne $layout) {
        Fail "$path layout/registry mismatch."
    }

    $forbiddenContent = '(?m)^(?:source_content(?:_in_record)?|secret_content(?:_in_record)?|absolute_local_paths_allowed|dependency_implementation_source_allowed)\s*=\s*true\s*$'
    if ([regex]::IsMatch($schema, $forbiddenContent)) {
        Fail "$path permits forbidden content."
    }
    if ([regex]::IsMatch($schema, '(?i)\bnull\b')) {
        Fail "$path uses null semantics; OptionalV1 ABSENT/PRESENT is required."
    }

    $canonicalGroups = @(Get-Array $schema 'canonical_field_order')
    if ($canonicalGroups.Count -lt 4 -or -not (Same-Sequence @($canonicalGroups[0..2]) @('schema_version', 'record_kind', 'status'))) {
        Fail "$path canonical_field_order must begin with schema_version, record_kind, status."
    }

    $fields = [regex]::Split($schema, '(?m)^\[\[field\]\]\s*$')
    $seenPath = [System.Collections.Generic.HashSet[string]]::new([StringComparer]::Ordinal)
    $seenOrder = [System.Collections.Generic.HashSet[int]]::new()
    $fieldPaths = [System.Collections.Generic.List[string]]::new()
    $orderedGroups = [System.Collections.Generic.List[string]]::new()
    $fieldKindMap = @{}
    $fieldRulesMap = @{}
    $lastGroup = ''
    $signatureCount = 0
    $signatureRefCount = 0

    for ($i = 1; $i -lt $fields.Count; $i++) {
        $fieldPath = Get-String $fields[$i] 'path'
        $fieldKind = Get-String $fields[$i] 'kind'
        $order = [int](Get-Int $fields[$i] 'canonical_order')
        $rules = @(Get-Array $fields[$i] 'rules')
        $totalFields++

        [void]$fieldPaths.Add($fieldPath)
        $fieldKindMap[$fieldPath] = $fieldKind
        $fieldRulesMap[$fieldPath] = $rules

        if (-not $seenPath.Add($fieldPath) -or -not $seenOrder.Add($order)) {
            Fail "$path duplicates field path/order at $fieldPath."
        }
        if (-not (Get-Bool $fields[$i] 'required') -or $rules.Count -eq 0) {
            Fail "$path field $fieldPath lacks required wrapper/rules."
        }
        if ($builtins -notcontains $fieldKind -and -not $typeMap.ContainsKey($fieldKind)) {
            Fail "$path field $fieldPath has unknown kind $fieldKind."
        }
        if ($fieldPath.EndsWith('[]') -and $typeMap.ContainsKey($fieldKind) -and [string]$typeRep[$fieldKind] -match '^list<') {
            Fail "$path field $fieldPath is list-of-list."
        }
        if ($fieldKind -ceq 'ClosedEnum' -and ($rules -join ' ') -notmatch '(?:equals_|one_of_|PASS_)') {
            Fail "$path field $fieldPath has an open enum."
        }
        if ($legacyAmbiguousRecordDigestPaths.Contains($fieldPath)) {
            Fail "$path retains ambiguous control-record digest field $fieldPath."
        }
        if ($fieldPath -match 'exact_record_file_sha256$' -and ($rules -join ' ') -notmatch 'not_signed_payload_sha256') {
            Fail "$path field $fieldPath does not distinguish complete-file from signed-payload identity."
        }

        $group = ($fieldPath -split '\.', 2)[0]
        if ($group.EndsWith('[]')) { $group = $group.Substring(0, $group.Length - 2) }
        if ($group -cne $lastGroup) {
            [void]$orderedGroups.Add($group)
            $lastGroup = $group
        }

        if ($fieldPath -ceq 'signature.record_sha256') {
            $signatureCount++
            if (($rules -join ' ') -notmatch '(?:exact_record_digest_rule|signed_payload_sha256_before_signature_table)') {
                Fail "$path has wrong embedded digest semantics."
            }
        }
        if ($fieldPath -match '^signature\.(?:signature_ref|[A-Za-z0-9_]+_signature_ref)$') {
            $signatureRefCount++
            $totalSignatureRefs++
            if ($fieldKind -cne 'ImmutableSignatureRef') {
                Fail "$path field $fieldPath must use ImmutableSignatureRef."
            }
            if (($rules -join ' ') -notmatch 'signed_payload_sha256_matches_record_sha256') {
                Fail "$path field $fieldPath does not bind the signed-payload digest."
            }
        }
    }

    if (-not (Same-Set @($seenOrder) @(1..($fields.Count - 1)))) {
        Fail "$path canonical orders are not contiguous."
    }
    if ($signatureCount -ne 1) {
        Fail "$path must define one signature.record_sha256."
    }
    if ($signatureRefCount -lt 1) {
        Fail "$path must define at least one immutable signature ref."
    }

    $expectedGroups = @($canonicalGroups | Select-Object -Skip 3)
    if (-not (Same-Sequence @($orderedGroups) $expectedGroups)) {
        Fail "$path field-group order differs from canonical_field_order."
    }

    switch ($kind) {
        'context_manifest_v1' {
            if ([string]$fieldKindMap['draft.exact_file_sha256'] -cne 'Sha256Digest') {
                Fail "$path must bind exact draft-file bytes explicitly."
            }
            if ($signatureRefCount -ne 2 -or [string]$fieldKindMap['signature.materializer_signature_ref'] -cne 'ImmutableSignatureRef' -or [string]$fieldKindMap['signature.reviewer_signature_ref'] -cne 'ImmutableSignatureRef') {
                Fail "$path must carry distinct materializer and reviewer signatures."
            }
        }
        'lease_event_v1' {
            if ([string]$fieldKindMap['event.reason_code'] -cne 'LeaseEventReasonCode') {
                Fail "$path event.reason_code must use LeaseEventReasonCode."
            }
            if ((Get-String $schema 'event_reason_type') -cne 'LeaseEventReasonCode') {
                Fail "$path does not bind the lease-event reason registry."
            }
        }
        'supersession_receipt_v1' {
            if ([string]$fieldKindMap['reason.code'] -cne 'SupersessionReasonCode') {
                Fail "$path reason.code must use SupersessionReasonCode."
            }
            if ((Get-String $schema 'reason_type') -cne 'SupersessionReasonCode') {
                Fail "$path does not bind the supersession reason registry."
            }
        }
        'package_submission_v1' {
            if (@($fieldRulesMap['public_handoff_candidate.configuration_digest']) -notcontains 'ABSENT_only_when_package_owns_no_configuration') {
                Fail "$path configuration absence rule is not explicit OptionalV1 semantics."
            }
        }
        'package_handoff_v1' {
            if ((Get-String $schema 'record_layout') -cne 'swarm/handoffs/<package>/<handoff_id>.toml') {
                Fail "$path uses API digest as record path identity."
            }
        }
    }

    $schemaFieldPaths[$kind] = @($fieldPaths)
    $schemaFieldKinds[$kind] = $fieldKindMap
}

if ((Get-Int $orchestration 'schema_version') -ne 5) {
    Fail 'Orchestration schema_version must be 5.'
}
$orchestrationPaths = [ordered]@{
    control_plane_schema_registry = $controlPath
    control_plane_type_registry = $typesPath
    control_plane_validator = 'tools/validate-ticket-issuance-contracts.ps1'
    control_plane_manual_workflow = $workflowPath
    issued_ticket_layout = 'swarm/tickets/<package>/<ticket_id>.toml'
    materialized_context_layout = 'swarm/context-manifests/<package>/<context_record_sha256>.toml'
    writer_lease_layout = 'swarm/leases/<package>/<lease_id>.toml'
    lease_event_layout = 'swarm/leases/<package>/events/<event_id>.toml'
    submission_layout = 'swarm/submissions/<package>/<submission_id>.toml'
    review_layout = 'swarm/reviews/<package>/<review_id>.toml'
    accepted_handoff_layout = 'swarm/handoffs/<package>/<handoff_id>.toml'
    supersession_layout = 'swarm/supersessions/<record_kind>/<receipt_id>.toml'
}
foreach ($entry in $orchestrationPaths.GetEnumerator()) {
    if ((Get-String $orchestration $entry.Key) -cne $entry.Value) {
        Fail "Orchestration path mismatch: $($entry.Key)"
    }
}
foreach ($entry in $reasonTypeBindings.GetEnumerator()) {
    if ((Get-String $orchestration $entry.Key) -cne $entry.Value) {
        Fail "Orchestration reason binding mismatch: $($entry.Key)"
    }
}
if ((Get-String $orchestration 'workflow_policy') -cne 'manual_only') {
    Fail 'Orchestration workflow policy must be manual_only.'
}

$leaseSection = Get-Section $orchestration 'lease'
$acceptanceSection = Get-Section $orchestration 'acceptance'
if (-not (Same-Sequence @(Get-Array $leaseSection 'required_fields') @($schemaFieldPaths['writer_lease_v1']))) {
    Fail 'Orchestration lease.required_fields differs from writer_lease_v1.'
}
if (-not (Same-Sequence @(Get-Array $acceptanceSection 'required_fields') @($schemaFieldPaths['package_handoff_v1']))) {
    Fail 'Orchestration acceptance.required_fields differs from package_handoff_v1.'
}
if (-not (Get-Bool $acceptanceSection 'ticket_lease_context_is_transitively_bound_through_submission')) {
    Fail 'Package handoff must bind ticket/lease/context transitively through the reviewed submission.'
}
Require-Tokens $orchestrationPath $orchestration @(
    'from = "REJECTED"',
    'to = "READY"',
    'new materialized context and assignment-ticket revision exist, launch/dependencies remain valid and no active lease exists'
)

if ((Get-Int $launch 'orchestration_registry_schema_version') -ne 5 -or (Get-String $launch 'orchestration_registry_path') -cne $orchestrationPath) {
    Fail 'Launch state does not pin orchestration schema v5.'
}

Require-Tokens $canonicalizationPath $canonicalization @(
    'signed_payload_sha256',
    'exact_record_file_sha256',
    'fixed-point self-hash',
    'UTF-8',
    'LF (`0A`)'
)
Require-Tokens $schemaReadmePath $schemaReadme @(
    'ticket.exact_record_file_sha256',
    'submission.exact_record_file_sha256',
    'Generic `ticket.sha256`, `submission.sha256`, `review.sha256`',
    'distinct materializer and reviewer signature refs'
)
Require-Tokens $operationsPath $operations @(
    'CONTROL_OPERATION_CONFLICT',
    'recover_control_operation',
    'RepositoryRelativeSafePath',
    'no_dot_or_dotdot_segment',
    'swarm/tickets/<package>/<ticket_id>.toml',
    'swarm/context-manifests/<package>/<context_record_sha256>.toml',
    'swarm/leases/<package>/<lease_id>.toml',
    'swarm/submissions/<package>/<submission_id>.toml',
    'swarm/reviews/<package>/<review_id>.toml',
    'swarm/handoffs/<package>/<handoff_id>.toml'
)
foreach ($failure in $expectedFailures) {
    if ($operations.IndexOf($failure, [StringComparison]::Ordinal) -lt 0) {
        Fail "$operationsPath omits failure: $failure"
    }
}
foreach ($legacy in @('<ticket-id>', '<context-digest>', '<lease-id>', '<event-id>', '<submission-id>', '<review-id>', '<handoff-id>', '<record-kind>', '<receipt-id>')) {
    if ($operations.IndexOf($legacy, [StringComparison]::Ordinal) -ge 0 -or $handoffReadme.IndexOf($legacy, [StringComparison]::Ordinal) -ge 0) {
        Fail "Legacy control-record placeholder remains: $legacy"
    }
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
    'validate-ticket-issuance-contracts.ps1'
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
    types = $typeMap.Count
    records = $schemas.Count
    fields = $totalFields
    signature_refs = $totalSignatureRefs
    workflows = $workflowFiles.Count
    issued_records = 0
    errors = @($errors)
}

if ($Json) {
    $result | ConvertTo-Json -Depth 6
}
else {
    Write-Host "Ticket issuance schemas: types=$($result.types) records=$($result.records) fields=$($result.fields) signatures=$($result.signature_refs) workflows=$($result.workflows)"
    foreach ($error in $errors) {
        Write-Host "ERROR: $error" -ForegroundColor Red
    }
    if ($result.ok) { Write-Host 'PASS' -ForegroundColor Green }
}

if (-not $result.ok) { exit 1 }
