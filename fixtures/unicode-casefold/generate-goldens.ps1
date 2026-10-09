param([string]$Output = "$PSScriptRoot/casefold_goldens.tsv")
$ErrorActionPreference = 'Stop'
$data = "$PSScriptRoot/CaseFolding-18.0.0.txt"
$expected = 'a004797658a457bec4dc11683e39f69249ea3b595b752dbea6721c4c9f587b0d'
if ((Get-FileHash -LiteralPath $data -Algorithm SHA256).Hash.ToLowerInvariant() -ne $expected) {
    throw 'Pinned Unicode data checksum mismatch'
}
$defaultMap = @{}; $turkicMap = @{}; $statuses = @{}
foreach ($line in [IO.File]::ReadAllLines($data)) {
    if ($line -match '^([0-9A-F]+); ([CFST]); ([0-9A-F ]+);') {
        $cp = [Convert]::ToInt32($matches[1], 16)
        $status = $matches[2]
        $out = @($matches[3].Trim().Split(' ') | ForEach-Object { [Convert]::ToInt32($_, 16) })
        if ($status -eq 'C' -or $status -eq 'F') {
            if ($defaultMap.ContainsKey($cp)) { throw 'Duplicate C/F mapping' }
            $defaultMap[$cp] = $out; $statuses[$cp] = $status
        } elseif ($status -eq 'T') { $turkicMap[$cp] = $out }
    }
}
function Scalars([int[]]$points) { return (($points | ForEach-Object { 'U+{0:X4}' -f $_ }) -join ' ') }
function Text([int[]]$points) { return (($points | ForEach-Object { [char]::ConvertFromUtf32($_) }) -join '') }
function Hex([byte[]]$bytes) { return (($bytes | ForEach-Object { '{0:X2}' -f $_ }) -join ' ') }
$inputs = @(
    '0041','0061','00DF','1E9E','03A3','03C3','03C2','0049','0130','0131','0069',
    '0307','0308','0301','0342','0958','25B6','00C0','0041 0300','0061 0300',
    '0055 0308','1E96','1FB7','1FD3','FB03','0390','0587','1DF95','10400','104B0','1E900',
    '0049 0073 0074 0061 006E 0062 0075 006C','0053 0074 0072 0061 00DF 0065',
    'FB03 0072 0065','',
    ((@('0430') * 512) -join ' '), ((@('0430') * 513) -join ' '),
    ((@('1E9E') * 256) -join ' '), ((@('1E9E') * 257) -join ' ')
)
$lines = [Collections.Generic.List[string]]::new()
$lines.Add("id`tinput_cp`tinput_utf8`tdefault_status`tdefault_cp`tdefault_utf8`tturkic_status`tturkic_cp`tturkic_utf8`tsource_start`tsource_end`tdefault_scalars`tdefault_bytes`tmax_term_scalars`tmax_term_bytes`texpected")
$number = 0
foreach ($inputHex in $inputs) {
    $number++
    [int[]]$inputPoints = @()
    if ($inputHex) { $inputPoints = @($inputHex.Split(' ') | ForEach-Object { [Convert]::ToInt32($_,16) }) }
    $defaultPoints = [Collections.Generic.List[int]]::new()
    $turkicPoints = [Collections.Generic.List[int]]::new()
    $defaultStatus = [Collections.Generic.List[string]]::new()
    $turkicStatus = [Collections.Generic.List[string]]::new()
    foreach ($cp in $inputPoints) {
        $fold = if ($defaultMap.ContainsKey($cp)) { $defaultMap[$cp] } else { @($cp) }
        $kind = if ($statuses.ContainsKey($cp)) { $statuses[$cp] } else { 'unchanged' }
        foreach ($out in $fold) { $defaultPoints.Add($out) }
        $defaultStatus.Add($kind)
        if ($turkicMap.ContainsKey($cp)) { $fold = $turkicMap[$cp]; $kind = 'T' }
        foreach ($out in $fold) { $turkicPoints.Add($out) }
        $turkicStatus.Add($kind)
    }
    $raw = [Text.Encoding]::UTF8.GetBytes((Text $inputPoints))
    $def = [Text.Encoding]::UTF8.GetBytes((Text $defaultPoints.ToArray()))
    $trk = [Text.Encoding]::UTF8.GetBytes((Text $turkicPoints.ToArray()))
    $result = if (!$inputPoints.Count) { 'EMPTY_NO_TERM' } elseif ($inputPoints.Count -gt 512 -or $defaultPoints.Count -gt 512 -or $def.Length -gt 1024) { 'TERM_TOO_LONG' } else { 'ACCEPT' }
    $id = 'g{0:D2}' -f $number
    $fields = @($id, (Scalars $inputPoints), (Hex $raw), ($defaultStatus -join ','), (Scalars $defaultPoints.ToArray()), (Hex $def), ($turkicStatus -join ','), (Scalars $turkicPoints.ToArray()), (Hex $trk), '0', [string]$raw.Length, [string]$defaultPoints.Count, [string]$def.Length, '512', '1024', $result)
    $lines.Add($fields -join "`t")
}
[IO.File]::WriteAllText($Output, (($lines -join "`n") + "`n"), [Text.UTF8Encoding]::new($false))
Write-Output "Generated $number goldens from pinned Unicode 18.0.0 data"
