[CmdletBinding()]
param(
    [string]$Root = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path,
    [switch]$Json
)

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest
$errors = [System.Collections.Generic.List[string]]::new()
function Fail([string]$Message) { $script:errors.Add($Message) }
function Read-Required([string]$Relative) {
    $path = Join-Path $Root $Relative
    if (-not (Test-Path $path -PathType Leaf)) { Fail "Missing file: $Relative"; return "" }
    [IO.File]::ReadAllText($path)
}
function Str([string]$Text, [string]$Key, [bool]$Required = $true) {
    $m = [regex]::Match($Text, ('(?m)^{0}\s*=\s*"([^"]*)"\s*$' -f [regex]::Escape($Key)))
    if (-not $m.Success) { if ($Required) { Fail "Missing string key '$Key'." }; return "" }
    $m.Groups[1].Value
}
function Int([string]$Text, [string]$Key) {
    $m = [regex]::Match($Text, ('(?m)^{0}\s*=\s*(\d+)\s*$' -f [regex]::Escape($Key)))
    if (-not $m.Success) { Fail "Missing integer key '$Key'."; return 0 }
    [int64]$m.Groups[1].Value
}
function Bool([string]$Text, [string]$Key) {
    $m = [regex]::Match($Text, ('(?m)^{0}\s*=\s*(true|false)\s*$' -f [regex]::Escape($Key)))
    if (-not $m.Success) { Fail "Missing boolean key '$Key'."; return $false }
    $m.Groups[1].Value -eq "true"
}
function Array([string]$Text, [string]$Key) {
    $m = [regex]::Match($Text, ('(?ms)^{0}\s*=\s*\[(.*?)\]' -f [regex]::Escape($Key)))
    if (-not $m.Success) { return @() }
    @([regex]::Matches($m.Groups[1].Value, '"([^"\r\n]+)"') | ForEach-Object { $_.Groups[1].Value })
}
function Require([string]$Path, [string]$Text, [string[]]$Tokens) {
    foreach ($token in $Tokens) {
        if (-not $Text.Contains($token, [StringComparison]::Ordinal)) { Fail "$Path missing token: $token" }
    }
}
function Same-Set([string[]]$A, [string[]]$B) {
    $a = @($A | Sort-Object -Unique); $b = @($B | Sort-Object -Unique)
    if ($a.Count -ne $b.Count) { return $false }
    for ($i = 0; $i -lt $a.Count; $i++) { if ($a[$i] -cne $b[$i]) { return $false } }
    $true
}

$packet = Read-Required "swarm/w7-lifecycle.toml"
$gate = Read-Required "swarm/gates-w7.toml"
$launch = Read-Required "swarm/launch-state.toml"
$baseline = Read-Required "qualification/lifecycle/baseline.toml"
$probes = Read-Required "qualification/lifecycle/probes.toml"
$qualification = Read-Required "qualification/lifecycle/W7_QUALIFICATION.md"
$settings = Read-Required "config/w7-lifecycle.toml"
$settingsDoc = Read-Required "docs/config/W7_LIFECYCLE_SETTINGS_1.0.md"
$handoff = Read-Required "docs/handoff/W7_IMPLEMENTATION_PACKET.md"
$fixtureOwners = Read-Required "qualification/lifecycle/fixture-owners.toml"

if ((Str $packet "status") -cne "BLOCKED") { Fail "W7 packet must remain BLOCKED." }
if ((Str $gate "status") -cne "BLOCKED") { Fail "W7 gate overlay must remain BLOCKED." }
if ((Int $launch "active_wave") -ne 0 -or (Str $launch "active_stage") -cne "P00") { Fail "Repository launch must remain P00/W0." }
if (-not (Same-Set @(Array $launch "authorized_packages") @("search-contracts"))) { Fail "Only search-contracts may be authorized." }

$blocks = [regex]::Split($packet, '(?m)^\[\[packet\]\]\s*$')
$packageSet = [System.Collections.Generic.HashSet[string]]::new([StringComparer]::Ordinal)
for ($i = 1; $i -lt $blocks.Count; $i++) {
    $block = $blocks[$i]
    $package = Str $block "package"
    if (-not $packageSet.Add($package)) { Fail "Duplicate W7 packet: $package" }
    foreach ($key in @("primary", "assignment")) {
        $relative = Str $block $key
        [void](Read-Required $relative)
    }
    $base = Str $block "base_functions" $false
    if ($base) { [void](Read-Required $base) }
    foreach ($relative in @(Array $block "config_packets")) { [void](Read-Required $relative) }
}
$expectedPackages = @("search-retention", "search-revision-store", "search-access", "search-handles", "search-continuation", "search-candidate-validator", "search-publication", "search-index-reclaimer")
if (-not (Same-Set @($packageSet) $expectedPackages)) { Fail "W7 package set mismatch." }

$retentionPath = "crates/search-runtime/search-retention/FUNCTIONS.md"
$revisionPath = "crates/search-source/search-revision-store/FUNCTIONS.md"
$retention = Read-Required $retentionPath
$revision = Read-Required $revisionPath
Require $retentionPath $retention @("## `collect_durable_roots`", "## `mark_reachable`", "## `execute_sweep_batch`", "### `install_purge_fence`", "### `finalize_purge_receipt`", "### `enter_restore_quarantine`", "### `commit_restore_admission`", "ordinary index-reclaim receipts", "SECURE_ERASE_NOT_GUARANTEED")
Require $revisionPath $revision @("## `enumerate_lifecycle_roots`", "## `apply_exact_object_deletion`", "## `install_purge_tombstone`", "## `enter_restore_quarantine`", "## `admit_restored_objects`", "Broad directory/prefix/content-digest deletion is forbidden")
foreach ($entry in @(
    @("crates/search-query/search-access/W7_HARDENING.md", @("LIVE_FENCE_PUBLISHED", "whole scoring/IDF leg", "## Checkpoints")),
    @("crates/search-query/search-handles/W7_HARDENING.md", @("Durable eligibility", "Expansion race hardening", "Unsaved/ephemeral handles never survive")),
    @("crates/search-query/search-continuation/W7_HARDENING.md", @("invalidate_lifecycle_scope", "Durable checkpoint eligibility", "Ephemeral records never restore")),
    @("crates/search-query/search-candidate-validator/W7_HARDENING.md", @("Checkpoint sequence", "CONTAMINATED_LEG", "purge tombstone dominates")),
    @("crates/search-index-qdrant/search-publication/W7_HARDENING.md", @("Invalidation-only publication", "Purge interaction", "Restore interaction")),
    @("crates/search-index-qdrant/search-index-reclaimer/W7_HARDENING.md", @("ordinary reclaimer accepts only", "security_purge = not_claimed", "restore quarantine generation"))
)) {
    $path = [string]$entry[0]; $text = Read-Required $path; Require $path $text ([string[]]$entry[1]
    )
}

if ((Str $baseline "status") -cne "DESIGNED_NOT_EXECUTED") { Fail "Baseline status must remain DESIGNED_NOT_EXECUTED." }
if (Bool $baseline "implementation_authorized") { Fail "W7 baseline must not authorize implementation." }
if ((Str $baseline "backup_provider") -cne "UNSELECTED") { Fail "Backup provider must remain UNSELECTED." }
foreach ($key in @("restrictive_state_monotonic", "ack_requires_live_snapshot_publication", "missing_invalidation_receipt_fails_closed", "durable_requires_immutable_revision", "durable_requires_retention_lease", "all_required_roots_mandatory", "active_pin_protection_mandatory", "exact_delete_readback_required", "logical_fence_before_ack", "tombstone_before_destructive_work", "tombstone_blocks_write_import_reindex_restore", "paired_manifest_required", "restore_starts_quarantined", "new_guarded_publication_required", "ordinary_reclaim_distinct", "object_sweep_distinct", "security_purge_distinct", "backup_deletion_distinct", "physical_secure_erase_distinct")) { if (-not (Bool $baseline $key)) { Fail "Required flag disabled: $key" } }
foreach ($key in @("candidate_only_cleanup_preserves_contaminated_ranking", "possession_grants_access", "durable_unsaved_allowed", "restored_old_token_serving_valid", "refcount_is_deletion_authority", "partial_mark_authorizes_sweep", "cancelled_mark_authorizes_sweep", "broad_object_delete_allowed", "ordinary_reclaim_receipt_satisfies_purge", "cas_sweep_receipt_satisfies_purge", "client_revocation_claims_client_data_deletion", "secure_erase_guaranteed", "redb_only_snapshot_can_serve", "qdrant_only_snapshot_can_serve", "serve_before_source_revalidation", "serve_before_access_residency_revalidation", "serve_before_purge_tombstone_revalidation", "old_visible_epoch_restored_current", "purged_material_may_reappear", "absence_implies_all_layers_complete")) { if (Bool $baseline $key) { Fail "Unsafe flag enabled: $key" } }

if ((Str $settings "status") -cne "schema-only" -or (Bool $settings "implementation_authorized")) { Fail "W7 settings must remain schema-only and unauthorized." }
Require "config/w7-lifecycle.toml" $settings @('mode = "COMMAND_ONLY"', 'name = "purge_command"', 'name = "restore_command"', 'remove_purge_tombstone = true', 'broad_object_delete = true', 'ordinary_reclaim_as_purge = true', 'secure_erase_overclaim = true', 'unpaired_restore = true', 'serve_quarantined_restore = true', 'restore_purged_material = true')
Require "docs/config/W7_LIFECYCLE_SETTINGS_1.0.md" $settingsDoc @("LOCKED", "COMMAND_ONLY", "paired manifest", "previous effective snapshot")
Require "qualification/lifecycle/W7_QUALIFICATION.md" $qualification @("## Mandatory properties", "### Retention and mark/sweep", "### Purge", "### Backup and restore", "## Stop conditions", "## Current disposition")
Require "docs/handoff/W7_IMPLEMENTATION_PACKET.md" $handoff @("Cross-package invariants", "Hard stop conditions", "60-probe corpus")
Require "qualification/lifecycle/fixture-owners.toml" $fixtureOwners @("retention_root_mark_sweep_fault_corpus", "purge_layer_non_resurrection_corpus", "paired_restore_quarantine_corpus", "producer_may_self_accept = false")

$probeBlocks = [regex]::Split($probes, '(?m)^\[\[probe\]\]\s*$')
$ids = [System.Collections.Generic.HashSet[string]]::new([StringComparer]::Ordinal)
$owners = [System.Collections.Generic.HashSet[string]]::new([StringComparer]::Ordinal)
$count = 0
for ($i = 1; $i -lt $probeBlocks.Count; $i++) {
    $block = $probeBlocks[$i]; $id = Str $block "id"; $owner = Str $block "owner"
    $count++; if (-not $ids.Add($id)) { Fail "Duplicate probe: $id" }; [void]$owners.Add($owner)
    if (-not (Bool $block "mandatory")) { Fail "Probe not mandatory: $id" }
    if ((Str $block "result") -cne "UNAVAILABLE") { Fail "Probe must remain UNAVAILABLE: $id" }
}
if ($count -ne 60) { Fail "Expected 60 W7 probes; parsed $count." }
$expectedOwners = @("search-access", "search-handles", "search-continuation", "search-candidate-validator", "search-revision-store", "search-retention")
if (-not (Same-Set @($owners) $expectedOwners)) { Fail "W7 probe owner set mismatch." }

$requiredEvidence = @(Array $packet "required_evidence")
$gateEvidence = @(Array $gate "required_evidence")
if (-not (Same-Set $requiredEvidence $gateEvidence)) { Fail "W7 packet and gate overlay evidence IDs differ." }
if ($gateEvidence.Count -ne 8) { Fail "Expected eight W7 evidence IDs." }

$result = [ordered]@{
    ok = ($errors.Count -eq 0)
    packets = $packageSet.Count
    probes = $count
    gate_evidence = $gateEvidence.Count
    backup_provider = Str $baseline "backup_provider"
    status = Str $packet "status"
    launch_stage = Str $launch "active_stage"
    launch_wave = Int $launch "active_wave"
    errors = @($errors)
}
if ($Json) { $result | ConvertTo-Json -Depth 6 }
else {
    Write-Host "ELIOT Search W7 lifecycle validation"
    Write-Host "packets=$($result.packets) probes=$($result.probes) status=$($result.status) backup=$($result.backup_provider)"
    foreach ($error in $errors) { Write-Host "ERROR: $error" -ForegroundColor Red }
    if ($result.ok) { Write-Host "PASS" -ForegroundColor Green }
}
if (-not $result.ok) { exit 1 }
