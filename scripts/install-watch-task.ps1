# install-watch-task.ps1 — durable supervisor for watch-host (OQ-12: cron heartbeat).
# Creates a Windows Scheduled Task running `watch-host.py --once` every N minutes,
# appending to a log under %LOCALAPPDATA%\okx-watch-host\.
#
# Notes:
#  - schtasks /create for the current user typically requires the user password,
#    or an Administrator terminal. If it prompts/fails, run from an elevated
#    PowerShell or supply -User/-Password.
#  - Alternative zero-admin supervisor: keep a terminal running
#        python scripts\watch-host.py
#    (loop mode) — fine for session-scoped operation.
#
# Usage:
#   powershell -ExecutionPolicy Bypass -File scripts\install-watch-task.ps1 [-IntervalMin 5] [-User $env:USERNAME]
$ErrorActionPreference = "Stop"
param([int]$IntervalMin = 5, [string]$User = "")

$repoRoot = Split-Path -Parent $PSScriptRoot
$py = (Get-Command python).Source
$script = Join-Path $repoRoot "scripts\watch-host.py"
$dataDir = Join-Path $env:LOCALAPPDATA "okx-watch-host"
New-Item -ItemType Directory -Force -Path $dataDir | Out-Null
$log = Join-Path $dataDir "watch.log"
$cmd = "cmd /c `"$py`" `"$script`" --once --event-dir `"$dataDir\events`" >> `"$log`" 2>&1"
$taskName = "okx-watch-host"

$args = @("/create", "/tn", $taskName, "/tr", $cmd, "/sc", "minute", "/mo", "$IntervalMin", "/f")
if ($User) { $args += @("/ru", $User) }
& schtasks.exe @args
if ($LASTEXITCODE -ne 0) {
    Write-Host "Task creation failed (exit $LASTEXITCODE). Retry from an Administrator terminal, or add -User with password prompt."
    exit $LASTEXITCODE
}
Write-Host "Task '$taskName' created: every ${IntervalMin} min -> $log"
Write-Host "Remove later with: schtasks /delete /tn $taskName /f"
