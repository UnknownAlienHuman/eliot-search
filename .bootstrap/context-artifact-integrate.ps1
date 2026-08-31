$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

if (-not (Test-Path .bootstrap/context-artifact-v1.00.b64 -PathType Leaf)) {
    Write-Host "Bootstrap already consumed; no integration action required."
    exit 0
}

git config --global core.autocrlf false
git config --global core.eol lf
$base = (git rev-parse HEAD).Trim()
$base | Set-Content "$env:RUNNER_TEMP/context-candidate-base.txt" -Encoding ascii

@'
from pathlib import Path, PurePosixPath
import base64
import hashlib
import io
import zipfile

expected = [
    ("context-artifact-v1.00.b64", 7000, "fcf31623fa76b408f9665d85b83bdb0f3423814778ccd5fad5605d5d28d4c720"),
    ("context-artifact-v1.010.b64", 1750, "462ca12ecf1785552948c4b5130474d024b6ff519325224f1d35b0f035d93b8b"),
    ("context-artifact-v1.011.b64", 1750, "2f65f99d27a2d5d7975a03647acd03a2c1baa22f5159bcfc1b7c3400b56fd075"),
    ("context-artifact-v1.012.b64", 1750, "a5a9ff6bceecb54757a7a0cfca91006141679674de7367357c3fb4e9a3fbfdc7"),
    ("context-artifact-v1.013.b64", 1750, "c230f221dacd228b3bc497ccd88dca29efc78057ddaf81a4ff340b400536ca30"),
    ("context-artifact-v1.02.b64", 14000, "31b65a1e9044a0a54934195ac94ab671533f3c911a872d0afc51e873336f480f"),
    ("context-artifact-v1.03.b64", 13060, "6eb2f70a94b91580a64dfde5ffe904cb00031577935ecbdfd1536c6391a74689"),
]
parts = []
for name, length, digest in expected:
    value = (Path(".bootstrap") / name).read_text(encoding="ascii")
    if len(value) != length:
        raise SystemExit(f"{name}: length {len(value)} != {length}")
    actual = hashlib.sha256(value.encode("ascii")).hexdigest()
    if actual != digest:
        raise SystemExit(f"{name}: sha256 {actual} != {digest}")
    parts.append(value)
archive_b64 = "".join(parts)
if len(archive_b64) != 41060:
    raise SystemExit(f"archive base64 length {len(archive_b64)} != 41060")
if hashlib.sha256(archive_b64.encode("ascii")).hexdigest() != "e6350840360a7fcdba8a7a0b9c709ad18715fdca061dd0632e52ac6d37473391":
    raise SystemExit("archive base64 digest mismatch")
archive = base64.b64decode(archive_b64, validate=True)
if hashlib.sha256(archive).hexdigest() != "944faf8d6cb8e1f433f71191b44318fc5fa810c3b1f8f50c4a7ca5935818e1bf":
    raise SystemExit("archive byte digest mismatch")
with zipfile.ZipFile(io.BytesIO(archive)) as bundle:
    if bundle.testzip() is not None:
        raise SystemExit("archive CRC validation failed")
    for info in bundle.infolist():
        pure = PurePosixPath(info.filename)
        if pure.is_absolute() or any(part in {"", ".", ".."} for part in pure.parts):
            raise SystemExit(f"unsafe archive path: {info.filename}")
        path = Path(*pure.parts)
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(bundle.read(info))
'@ | Set-Content "$env:RUNNER_TEMP/materialize-context-candidate.py" -Encoding utf8

function Remove-PythonCaches {
    Get-ChildItem -Recurse -Directory -Filter __pycache__ -ErrorAction SilentlyContinue |
        Remove-Item -Recurse -Force
    Get-ChildItem -Recurse -File -Include *.pyc,*.pyo -ErrorAction SilentlyContinue |
        Remove-Item -Force
}

function Materialize-Archive {
    python "$env:RUNNER_TEMP/materialize-context-candidate.py"
}

Materialize-Archive
Remove-Item .github/workflows/_temp-context-artifact-candidate-v1-retry.yml -Force
Remove-Item .bootstrap -Recurse -Force
Remove-PythonCaches
git diff --check

git config user.name "github-actions[bot]"
git config user.email "41898282+github-actions[bot]@users.noreply.github.com"
git add -A
git commit -m "Temporary qualified context artifact candidate tree"

python -m py_compile `
    tools/build-context-artifact-candidate.py `
    tools/context_artifact_builder_v1/__init__.py `
    tools/context_artifact_builder_v1/core.py `
    tools/context_artifact_builder_v1/bundle.py `
    tools/context_artifact_builder_v1/extract.py `
    tools/context_artifact_builder_v1/build.py `
    tools/validate-context-artifact-candidate.py `
    qualification/context-artifact/test_context_artifact_candidate_v1.py
python qualification/context-artifact/test_context_artifact_candidate_v1.py
./tools/validate-context-artifact-candidate.ps1 -Json
./tools/validate-ticket-issuance-plan.ps1 -Json
./tools/validate-swarm.ps1 -Json
./tools/validate-p00-ticket-drafts.ps1 -Json
./tools/validate-ticket-issuance-contracts.ps1 -Json
./tools/validate-p00-foundation-acceptance.ps1 -Json

$format = (git rev-parse --show-object-format).Trim()
$qualifiedHead = (git rev-parse HEAD).Trim()
$qualifiedBase = "${format}:${qualifiedHead}"
./tools/build-context-artifact-candidate.ps1 `
    -Package search-contracts `
    -BaseCommit $qualifiedBase `
    -OutputRoot artifacts/context-artifact-candidates/workflow `
    -PrintResult
$candidate = Get-ChildItem artifacts/context-artifact-candidates/workflow/search-contracts/*.json |
    Select-Object -First 1
$record = Get-Content $candidate.FullName -Raw | ConvertFrom-Json
if ($record.status -ne 'ARTIFACT_CANDIDATE_NOT_STORED_NOT_SIGNED') {
    throw "Unexpected candidate status: $($record.status)"
}
if ($record.reason_codes.Count -ne 0) {
    throw "Unexpected candidate reasons: $($record.reason_codes -join ',')"
}
if ($record.control_record_mutations.Count -ne 0) {
    throw "Candidate emitted a control-record mutation."
}
if ($record.manifest_projection.schema_instance -ne $false) {
    throw "Candidate projected a context manifest schema instance."
}
foreach ($property in $record.authority.PSObject.Properties) {
    if ($property.Value -ne $false) {
        throw "Candidate enabled authority field: $($property.Name)"
    }
}

# Rebuild from the exact remote base so the pushed commit changes no workflow path.
git reset --hard $base
Materialize-Archive
New-Item -ItemType Directory -Path .bootstrap-final -Force | Out-Null
Copy-Item .github/workflows/context-artifact-candidate.yml `
    .bootstrap-final/context-artifact-candidate.yml.txt
Remove-Item .github/workflows/context-artifact-candidate.yml -Force
Remove-Item .bootstrap -Recurse -Force
Remove-PythonCaches
git diff --check
git add -A
git commit -m "Add deterministic context artifact candidate builder"
git push origin HEAD:refs/heads/prep/context-artifact-candidate-v1
