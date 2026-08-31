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
    $match = [regex]::Match($Text, ('(?ms)^{0}[ \t]*=[ \t]*\[(.*?)\][ \t]*\r?$' -f [regex]::Escape($Key)))
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

function Assert-UniqueNonEmpty([string]$Owner, [string]$Name, [object[]]$Values, [int]$ExpectedCount) {
    $items = @($Values | ForEach-Object { [string]$_ })
    if ($items.Count -ne $ExpectedCount) {
        Fail "$Owner $Name count $($items.Count) != $ExpectedCount."
    }
    if (@($items | Where-Object { [string]::IsNullOrWhiteSpace($_) }).Count -ne 0) {
        Fail "$Owner $Name contains an empty value."
    }
    if (@($items | Sort-Object -Unique).Count -ne $items.Count) {
        Fail "$Owner $Name contains duplicates."
    }
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

if ((Number $ticketManifest 'schema_version') -ne 2 -or (Number $ticketManifest 'ticket_draft_schema_version') -ne 2 -or (Number $ticketManifest 'context_draft_manifest_schema_version') -ne 2) {
    Fail 'Ticket draft manifest must pin ticket/context draft schema v2.'
}
if ((Value $ticketManifest 'status') -cne 'DRAFT_ONLY_NOT_ISSUED' -or (Number $ticketManifest 'draft_count') -ne 3) {
    Fail 'Ticket draft manifest identity/count mismatch.'
}
foreach ($zero in @('issued_ticket_count', 'active_lease_count', 'submission_count', 'accepted_review_count', 'package_handoff_count', 'wave_receipt_count')) {
    if ((Number $ticketManifest $zero) -ne 0) { Fail "$zero must be zero." }
}
foreach ($required in @(
    @('context_draft_manifest', 'swarm/context-drafts/manifest.toml'),
    @('assignment_ticket_template', 'swarm/ASSIGNMENT_TICKET_TEMPLATE.md'),
    @('writer_lease_template', 'swarm/WRITER_LEASE_TEMPLATE.md'),
    @('context_manifest_template', 'swarm/CONTEXT_MANIFEST_TEMPLATE.md'),
    @('submission_template', 'swarm/SUBMISSION_TEMPLATE.md'),
    @('review_template', 'swarm/REVIEW_RECEIPT_TEMPLATE.md')
)) {
    if ((Value $ticketManifest $required[0]) -cne $required[1]) {
        Fail "Ticket manifest path mismatch: $($required[0])"
    }
}
$ticketInvariants = Section $ticketManifest 'invariants'
foreach ($unsafe in @('draft_is_orchestration_state', 'draft_may_authorize', 'draft_may_create_lease', 'draft_may_contain_lease_identity', 'draft_may_be_writer_acknowledged')) {
    if (Flag $ticketInvariants $unsafe) { Fail "Unsafe ticket manifest invariant: $unsafe" }
}
foreach ($required in @('draft_uses_distinct_signed_payload_and_exact_file_digest_slots', 'issued_ticket_requires_new_record', 'issued_ticket_requires_exact_base_commit', 'issued_ticket_requires_materialized_context', 'issued_ticket_requires_writer_and_reviewer', 'conditional_ticket_requires_accepted_dependency_handoffs')) {
    if (-not (Flag $ticketInvariants $required)) { Fail "Required ticket manifest invariant disabled: $required" }
}

if ((Number $contextManifest 'schema_version') -ne 2 -or (Number $contextManifest 'context_draft_schema_version') -ne 2 -or (Value $contextManifest 'status') -cne 'NON_CLAIMABLE_CONTEXT_DRAFTS' -or (Number $contextManifest 'draft_count') -ne 3 -or (Number $contextManifest 'materialized_context_count') -ne 0) {
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
$contextInvariants = Section $contextManifest 'invariants'
foreach ($unsafe in @('architecture_master_allowed', 'dependency_implementation_source_allowed', 'materialized_context_may_be_amended', 'p00_exception_may_add_ad_hoc_sources')) {
    if (Flag $contextInvariants $unsafe) { Fail "Unsafe context manifest invariant: $unsafe" }
}
foreach ($required in @('base_commit_required_at_materialization', 'per_source_sha256_required', 'registry_selector_must_match_exactly_one_record', 'accepted_handoff_digests_required_when_declared', 'canonical_order_required', 'manifest_and_artifact_identities_are_distinct', 'p00_exception_requires_manifest_closed_exact_pack')) {
    if (-not (Flag $contextInvariants $required)) { Fail "Required context manifest invariant disabled: $required" }
}
if ((Number $contextInvariants 'p00_exception_writer_visible_artifact_count') -ne 1) {
    Fail 'P00 exact-pack exception must still emit one writer-visible artifact.'
}

$p00RequiredNames = @(Array $p00Manifest 'required_files')
Assert-UniqueNonEmpty 'P00 manifest' 'required_files' $p00RequiredNames 12
$p00RequiredPaths = @($p00RequiredNames | ForEach-Object { "docs/contracts/p00/$_" })

$searchContractsSourcesList = [System.Collections.Generic.List[string]]::new()
foreach ($source in @(
    'AGENTS.md',
    'crates/search-contracts/AGENTS.md',
    'docs/handoff/AUTHORITY_MAP.md',
    'swarm/ASSIGNMENT_PROTOCOL.md',
    'swarm/assignments/search-contracts.md',
    'docs/handoff/P00_BOOTSTRAP.md',
    $p00RequiredPaths[0],
    'docs/contracts/p00/manifest.toml'
)) {
    [void]$searchContractsSourcesList.Add([string]$source)
}
for ($i = 1; $i -lt $p00RequiredPaths.Count; $i++) {
    [void]$searchContractsSourcesList.Add([string]$p00RequiredPaths[$i])
}
$searchContractsSources = $searchContractsSourcesList.ToArray()

$searchDomainSources = @(
    'AGENTS.md',
    'crates/search-domain/AGENTS.md',
    'docs/handoff/AUTHORITY_MAP.md',
    'swarm/ASSIGNMENT_PROTOCOL.md',
    'swarm/assignments/search-domain.md',
    'docs/handoff/P00_BOOTSTRAP.md',
    'docs/contracts/p00/manifest.toml',
    'docs/contracts/p00/CANONICAL_TYPES.md',
    'docs/contracts/p00/SUPPORT_SCHEMAS.md',
    'docs/contracts/p00/CONTRACT_CHALLENGES.md',
    'docs/contracts/p00/SOURCE_GRAPH.md',
    'docs/contracts/p00/QUERY_AND_RESULTS.md',
    'docs/contracts/p00/RECIPE_RESULTS.md',
    'docs/contracts/p00/PROTOCOL_AND_LIFECYCLE.md',
    'docs/contracts/p00/REASON_CODES.md'
)

$searchPortsSources = @(
    'AGENTS.md',
    'crates/search-ports/AGENTS.md',
    'docs/handoff/AUTHORITY_MAP.md',
    'swarm/ASSIGNMENT_PROTOCOL.md',
    'swarm/assignments/search-ports.md',
    'docs/handoff/P00_BOOTSTRAP.md',
    'docs/contracts/p00/manifest.toml',
    'docs/contracts/p00/CANONICAL_TYPES.md',
    'docs/contracts/p00/TYPE_REGISTRY.md',
    'docs/contracts/p00/SUPPORT_SCHEMAS.md',
    'docs/contracts/p00/CONTRACT_CHALLENGES.md',
    'docs/contracts/p00/PORT_OPERATIONS.md',
    'docs/contracts/p00/PROTOCOL_AND_LIFECYCLE.md',
    'docs/contracts/p00/REASON_CODES.md'
)

$expected = [ordered]@{
    'search-contracts' = @{
        Launch = 'AUTHORIZED'
        Precondition = 'CURRENTLY_PRESENT'
        IssuanceStatus = 'BLOCKED_ON_IDENTITY_DIGEST_AND_CONTEXT_FREEZE'
        Scope = 'crates/search-contracts/**'
        Soft = 8000
        Handoffs = 0
        CeilingClass = 'P00_EXACT_CONTRACT_PACK'
        Sources = $searchContractsSources
        Fragments = @(
            'swarm/crates.toml::package[name=search-contracts]',
            'swarm/function-packets.toml::foundation[package=search-contracts]',
            'swarm/stages.toml::stage[id=W0]',
            'swarm/launch-state.toml::authorized_packages[search-contracts]'
        )
        HandoffSlots = @()
        UnavailableChecks = @('real_stable_windows_toolchain', 'generated_Cargo_lock', 'cargo_fmt_workspace', 'cargo_check_workspace', 'cargo_deny_policy')
        Outputs = 6
        Evidence = 15
        Issuance = 7
    }
    'search-domain' = @{
        Launch = 'CONDITIONAL'
        Precondition = 'ACCEPTED_SEARCH_CONTRACTS_HANDOFF_REQUIRED'
        IssuanceStatus = 'BLOCKED_ON_CONTRACTS_HANDOFF_IDENTITY_DIGEST_AND_CONTEXT_FREEZE'
        Scope = 'crates/search-domain/**'
        Soft = 7000
        Handoffs = 1
        CeilingClass = 'ORDINARY'
        Sources = $searchDomainSources
        Fragments = @(
            'swarm/crates.toml::package[name=search-domain]',
            'swarm/function-packets.toml::foundation[package=search-domain]',
            'swarm/stages.toml::stage[id=W0]',
            'swarm/launch-state.toml::conditional_packages[search-domain]',
            'swarm/launch-state.toml::conditional_activation.search-domain'
        )
        HandoffSlots = @('search-contracts::accepted_package_and_api_handoff')
        UnavailableChecks = @('accepted_search_contracts_handoff', 'real_stable_windows_toolchain', 'generated_Cargo_lock', 'workspace_integration_tests')
        Outputs = 5
        Evidence = 9
        Issuance = 9
    }
    'search-ports' = @{
        Launch = 'CONDITIONAL'
        Precondition = 'ACCEPTED_SEARCH_CONTRACTS_HANDOFF_REQUIRED'
        IssuanceStatus = 'BLOCKED_ON_CONTRACTS_HANDOFF_IDENTITY_DIGEST_AND_CONTEXT_FREEZE'
        Scope = 'crates/search-ports/**'
        Soft = 5500
        Handoffs = 1
        CeilingClass = 'ORDINARY'
        Sources = $searchPortsSources
        Fragments = @(
            'swarm/crates.toml::package[name=search-ports]',
            'swarm/function-packets.toml::foundation[package=search-ports]',
            'swarm/stages.toml::stage[id=W0]',
            'swarm/launch-state.toml::conditional_packages[search-ports]',
            'swarm/launch-state.toml::conditional_activation.search-ports'
        )
        HandoffSlots = @('search-contracts::accepted_package_and_api_handoff')
        UnavailableChecks = @('accepted_search_contracts_handoff', 'real_stable_windows_toolchain', 'generated_Cargo_lock', 'public_port_conformance_integration')
        Outputs = 5
        Evidence = 8
        Issuance = 9
    }
}

$ticketDraftBlocks = [regex]::Split($ticketManifest, '(?m)^\[\[draft\]\]\s*$')
$ticketEntries = @{}
for ($i = 1; $i -lt $ticketDraftBlocks.Count; $i++) {
    $package = Value $ticketDraftBlocks[$i] 'package'
    if ([string]::IsNullOrWhiteSpace($package)) { continue }
    if ($ticketEntries.ContainsKey($package)) {
        Fail "Duplicate ticket manifest package: $package"
        continue
    }
    $ticketEntries[$package] = $ticketDraftBlocks[$i]
}

$contextDraftBlocks = [regex]::Split($contextManifest, '(?m)^\[\[draft\]\]\s*$')
$contextEntries = @{}
for ($i = 1; $i -lt $contextDraftBlocks.Count; $i++) {
    $package = Value $contextDraftBlocks[$i] 'package'
    if ([string]::IsNullOrWhiteSpace($package)) { continue }
    if ($contextEntries.ContainsKey($package)) {
        Fail "Duplicate context manifest package: $package"
        continue
    }
    $contextEntries[$package] = $contextDraftBlocks[$i]
}
if (-not (Same @($ticketEntries.Keys) @($expected.Keys)) -or -not (Same @($contextEntries.Keys) @($expected.Keys))) {
    Fail 'Draft package set mismatch.'
}

foreach ($entry in $expected.GetEnumerator()) {
    $package = [string]$entry.Key
    $spec = $entry.Value
    $ticketPath = "swarm/ticket-drafts/p00/$package.toml"
    $contextPath = "swarm/context-drafts/p00/$package.toml"
    $ticket = Read-File $ticketPath
    $context = Read-File $contextPath

    $ticketEntry = [string]$ticketEntries[$package]
    if ((Value $ticketEntry 'path') -cne $ticketPath -or (Value $ticketEntry 'launch_class') -cne $spec.Launch -or (Value $ticketEntry 'status') -cne 'DRAFT_ONLY_NOT_ISSUED' -or (Flag $ticketEntry 'claimable')) {
        Fail "$package ticket manifest entry mismatch."
    }
    $contextEntry = [string]$contextEntries[$package]
    if ((Value $contextEntry 'path') -cne $contextPath -or (Value $contextEntry 'status') -cne 'UNMATERIALIZED_DRAFT' -or (Flag $contextEntry 'claimable') -or (Value $contextEntry 'source_ceiling_class') -cne $spec.CeilingClass) {
        Fail "$package context manifest entry mismatch."
    }

    if ((Number $ticket 'schema_version') -ne 2 -or (Value $ticket 'record_kind') -cne 'assignment_ticket_draft' -or (Value $ticket 'status') -cne 'DRAFT_ONLY_NOT_ISSUED') {
        Fail "$package ticket identity/schema mismatch."
    }
    foreach ($unsafe in @('claimable', 'authorizes_implementation', 'creates_lease', 'may_be_writer_acknowledged')) {
        if (Flag $ticket $unsafe) { Fail "$package ticket enables $unsafe." }
    }
    if ((Value $ticket 'package') -cne $package -or (Value $ticket 'stage') -cne 'W0' -or (Value $ticket 'phase') -cne 'P00' -or (Number $ticket 'wave') -ne 0) {
        Fail "$package ticket stage identity mismatch."
    }
    if ((Value $ticket 'launch_class') -cne $spec.Launch -or (Value $ticket 'launch_precondition') -cne $spec.Precondition -or (Value $ticket 'issuance_status') -cne $spec.IssuanceStatus) {
        Fail "$package launch/issuance classification mismatch."
    }

    $identity = Section $ticket 'unresolved_identity'
    if ([regex]::IsMatch($identity, '(?m)^lease_id\s*=') -or [regex]::IsMatch($identity, '(?m)^ticket_canonical_digest\s*=')) {
        Fail "$package ticket draft contains legacy lease or ambiguous digest identity."
    }
    foreach ($key in @('ticket_id', 'writer', 'reviewer')) {
        if ((Value $identity $key) -cne 'UNASSIGNED') { Fail "$package $key is prematurely assigned." }
    }
    if ((Value $identity 'issued_at') -ne '' -or (Value $identity 'integration_signature_ref') -ne '') {
        Fail "$package contains premature issue/signature metadata."
    }
    foreach ($key in @('base_commit', 'branch_or_worktree')) {
        if ((Value $identity $key) -cne 'UNSELECTED') { Fail "$package $key is prematurely selected." }
    }
    foreach ($key in @('ticket_signed_payload_sha256', 'ticket_exact_record_file_sha256')) {
        if ((Value $identity $key) -cne 'UNAVAILABLE') { Fail "$package $key is prematurely available." }
    }

    $fence = Section $ticket 'repository_fence'
    if ((Value $fence 'repository') -cne 'UnknownAlienHuman/eliot-search' -or (Value $fence 'write_scope') -cne $spec.Scope -or (Value $fence 'feature_profile') -cne 'P00_FOUNDATION') {
        Fail "$package repository fence mismatch."
    }
    foreach ($pair in @(
        @('package_registry_path', 'swarm/crates.toml'),
        @('function_registry_path', 'swarm/function-packets.toml'),
        @('stage_registry_path', 'swarm/stages.toml'),
        @('launch_state_path', 'swarm/launch-state.toml')
    )) {
        if ((Value $fence $pair[0]) -cne $pair[1] -or -not (Test-Path (Join-Path $Root $pair[1]) -PathType Leaf)) {
            Fail "$package registry fence mismatch: $($pair[0])"
        }
    }
    if ((Value $fence 'registry_digests') -cne 'UNRESOLVED_AT_ISSUANCE') {
        Fail "$package registry digests are prematurely resolved."
    }

    $ticketContext = Section $ticket 'context'
    if ((Value $ticketContext 'context_draft') -cne $contextPath -or (Value $ticketContext 'architecture_access') -cne 'exception-only' -or (Number $ticketContext 'writer_visible_artifact_count') -ne 1) {
        Fail "$package ticket context fence mismatch."
    }
    foreach ($key in @('context_manifest_ref', 'context_artifact_ref', 'context_artifact_sha256')) {
        if ((Value $ticketContext $key) -cne 'UNAVAILABLE') { Fail "$package $key is prematurely available." }
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
        if (-not (Same-Sequence $requiredDeps @('search-contracts')) -or $acceptedDeps.Count -ne 0 -or (Value $deps 'required_contract_commit') -cne 'UNSELECTED' -or (Value $deps 'required_contract_api_schema_digest') -cne 'UNAVAILABLE' -or (Value $deps 'status') -cne 'UNAVAILABLE') {
            Fail "$package must remain blocked on an unresolved accepted search-contracts handoff."
        }
    }

    $limits = Section $ticket 'limits'
    if ((Number $limits 'soft_src_lines') -ne $spec.Soft -or (Number $limits 'split_review_total_lines') -ne 8500 -or (Number $limits 'hard_total_lines') -ne 10000 -or -not (Flag $limits 'one_active_writer')) {
        Fail "$package limits mismatch."
    }

    $requiredOutputs = @(Array $ticket 'required_outputs')
    $requiredEvidence = @(Array $ticket 'required_evidence')
    $issuanceRequirements = @(Array $ticket 'issuance_requirements')
    Assert-UniqueNonEmpty $package 'required_outputs' $requiredOutputs ([int]$spec.Outputs)
    Assert-UniqueNonEmpty $package 'required_evidence' $requiredEvidence ([int]$spec.Evidence)
    Assert-UniqueNonEmpty $package 'issuance_requirements' $issuanceRequirements ([int]$spec.Issuance)
    foreach ($required in @('issue_assignment_ticket_as_new_record', 'issue_writer_lease_as_new_record_after_ticket_readback', 'record_writer_acknowledgement_as_append_only_lease_event')) {
        if ($issuanceRequirements -notcontains $required) { Fail "$package issuance requirements omit $required." }
    }
    if ($issuanceRequirements -contains 'issue_ticket_and_writer_lease_as_new_records') {
        Fail "$package retains the legacy combined ticket/lease issuance requirement."
    }

    if ((Number $context 'schema_version') -ne 2 -or (Value $context 'record_kind') -cne 'writer_context_draft' -or (Value $context 'status') -cne 'UNMATERIALIZED_DRAFT') {
        Fail "$package context identity/schema mismatch."
    }
    foreach ($unsafe in @('claimable', 'authorizes_implementation')) {
        if (Flag $context $unsafe) { Fail "$package context enables $unsafe." }
    }
    if ((Value $context 'package') -cne $package -or (Value $context 'stage') -cne 'W0' -or (Value $context 'phase') -cne 'P00' -or (Number $context 'wave') -ne 0 -or (Value $context 'base_commit') -cne 'UNSELECTED') {
        Fail "$package context stage/base identity mismatch."
    }
    if ([regex]::IsMatch($context, '(?m)^materialized_context_(?:ref|sha256)\s*=')) {
        Fail "$package context retains ambiguous legacy output identity."
    }
    foreach ($key in @('materialized_context_manifest_ref', 'materialized_context_record_sha256', 'materialized_context_artifact_ref', 'materialized_context_artifact_sha256')) {
        if ((Value $context $key) -cne 'UNAVAILABLE') { Fail "$package $key is prematurely available." }
    }
    if ((Value $context 'materialization_mode') -cne 'canonical_concatenated_bundle' -or (Number $context 'writer_visible_artifact_count') -ne 1) {
        Fail "$package context materialization contract mismatch."
    }

    $canonicalization = Section $context 'canonicalization'
    if ((Value $canonicalization 'encoding') -cne 'UTF-8' -or (Value $canonicalization 'line_endings') -cne 'LF' -or (Value $canonicalization 'path_header_format') -cne '--- repository-path: <path> ---' -or (Value $canonicalization 'registry_header_format') -cne '--- registry-selector: <path>::<selector> ---') {
        Fail "$package context canonicalization format mismatch."
    }
    foreach ($required in @('preserve_declared_order', 'record_source_sha256', 'record_fragment_sha256')) {
        if (-not (Flag $canonicalization $required)) { Fail "$package context disables $required." }
    }

    $sources = @(Array $context 'source_files')
    $fragments = @(Array $context 'registry_fragments')
    $handoffs = @(Array $context 'accepted_handoff_slots')
    if ((Number $context 'source_file_count') -ne $sources.Count -or (Number $context 'registry_fragment_count') -ne $fragments.Count -or (Number $context 'accepted_handoff_slot_count') -ne $handoffs.Count) {
        Fail "$package context counts mismatch."
    }
    if (-not (Same-Sequence $sources $spec.Sources)) {
        Fail "$package context source list/order differs from the exact bounded draft."
    }
    if (-not (Same-Sequence $fragments $spec.Fragments)) {
        Fail "$package context registry fragment list/order differs from the exact bounded draft."
    }
    if (-not (Same-Sequence $handoffs $spec.HandoffSlots)) {
        Fail "$package context accepted-handoff slots differ from the exact bounded draft."
    }
    if (-not (Same-Sequence @(Array $context 'required_unavailable_checks') $spec.UnavailableChecks)) {
        Fail "$package context unavailable-check list/order differs from the exact bounded draft."
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

    foreach ($source in $sources) {
        if (-not (Test-Path (Join-Path $Root $source) -PathType Leaf)) {
            Fail "$package context source missing: $source"
        }
        if ($source -like 'docs/architecture/*' -or $source -match '^(crates|bins)/.+/src/') {
            Fail "$package context includes forbidden implementation/architecture source: $source"
        }
    }
    $forbiddenPaths = @(Array $context 'forbidden_paths')
    foreach ($required in @('docs/architecture/**', 'bins/**', 'swarm/tickets/**', 'swarm/leases/**', 'swarm/submissions/**', 'swarm/reviews/**')) {
        if ($forbiddenPaths -notcontains $required) { Fail "$package context forbidden paths omit $required." }
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
    ticket_draft_schema = 2
    context_draft_schema = 2
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
    Write-Host "P00 draft control validation: drafts=3 contexts=3 draft-schema=v2 ordinary=$ordinarySourceCeiling exception=$p00SourceCeiling issued=0 leases=0 workflows=$($result.workflows) launch=P00/W0"
    foreach ($error in $errors) {
        Write-Host "ERROR: $error" -ForegroundColor Red
    }
    if ($result.ok) { Write-Host 'PASS' -ForegroundColor Green }
}

if (-not $result.ok) { exit 1 }
