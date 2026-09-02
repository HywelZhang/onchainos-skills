# sync-to-hermes.ps1 - copy fork skills/ into Hermes skills dir (whole-dir copy).
# Why NOT `hermes skills install <url>`: it drops _shared/ subdirs and breaks
# cross-skill references. Backups go OUTSIDE the scanned skills root.
# Usage:  powershell -ExecutionPolicy Bypass -File scripts/sync-to-hermes.ps1
$ErrorActionPreference = "Stop"

$repoRoot = Split-Path -Parent $PSScriptRoot
$forkSkills = Join-Path $repoRoot "skills"

$hermesSkills = $env:HERMES_SKILLS_DIR
if (-not $hermesSkills) {
    $hermesSkills = Join-Path $env:LOCALAPPDATA "hermes\skills"
}
if (-not (Test-Path $hermesSkills)) {
    Write-Error "Cannot locate Hermes skills dir: $hermesSkills (set HERMES_SKILLS_DIR)"
}
$okxDest = Join-Path $hermesSkills "okx"
New-Item -ItemType Directory -Force -Path $okxDest | Out-Null

$ts = Get-Date -Format "yyyyMMdd-HHmmss"
if (Get-ChildItem "$okxDest\*\SKILL.md" -ErrorAction SilentlyContinue) {
    $bk = Join-Path (Split-Path -Parent $hermesSkills) "skills-backup-$ts"
    New-Item -ItemType Directory -Force -Path $bk | Out-Null
    Copy-Item -Recurse -Force "$okxDest\*" $bk
    Write-Host "backup: $okxDest -> $bk"
}

$fail = $false
Get-ChildItem $forkSkills -Directory | ForEach-Object {
    $name = $_.Name
    if (-not (Test-Path (Join-Path $_.FullName "SKILL.md"))) { return }
    $dest = Join-Path $okxDest $name
    if (Test-Path $dest) { Remove-Item -Recurse -Force $dest }
    Copy-Item -Recurse -Force $_.FullName $dest
    $srcN = (Get-ChildItem -Recurse -File $_.FullName).Count
    $dstN = (Get-ChildItem -Recurse -File $dest).Count
    if ($srcN -ne $dstN) { Write-Host "MISMATCH $name: src=$srcN dst=$dstN"; $fail = $true }
    else { Write-Host "ok  $name ($dstN files)" }
}

Write-Host "synced: $forkSkills -> $okxDest"
if ($fail) { exit 1 }
