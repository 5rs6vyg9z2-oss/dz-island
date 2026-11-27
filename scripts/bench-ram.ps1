<#
.SYNOPSIS
  Launches DZ Island, lets it settle, samples its memory, then quits it cleanly.

.DESCRIPTION
  DZ Island hides the Windows taskbar and registers app bars, so it must be shut
  down through its own exit path. This script sends the normal close request
  (which restores the taskbar) and falls back to restoring the taskbar itself
  if the process has to be killed.

  Use -KeepRunning to inspect the app afterwards, or -Exe to compare builds.

.PARAMETER Exe
  Path to dz-island.exe. Default: src-tauri\target\release\dz-island.exe

.PARAMETER WarmupSeconds
  Seconds to wait after launch before sampling. Default: 20

.PARAMETER Samples
  Number of samples to average. Default: 5

.PARAMETER IntervalSeconds
  Seconds between samples. Default: 3

.PARAMETER Detail
  Print the per-process breakdown of the last sample.

.PARAMETER KeepRunning
  Leave DZ Island running afterwards instead of quitting it.

.EXAMPLE
  .\scripts\bench-ram.ps1 -Detail
  .\scripts\bench-ram.ps1 -Samples 10 -IntervalSeconds 2 -KeepRunning
#>
[CmdletBinding()]
param(
  [string]$Exe = 'src-tauri\target\release\dz-island.exe',
  [int]$WarmupSeconds = 20,
  [int]$Samples = 5,
  [double]$IntervalSeconds = 3,
  [switch]$Detail,
  [switch]$KeepRunning
)

$ErrorActionPreference = 'Stop'
. "$PSScriptRoot\dz-island-mem.ps1"

$exePath = (Resolve-Path $Exe).Path

if (Get-Process -Name 'dz-island' -ErrorAction SilentlyContinue) {
  Write-Host "DZ Island is already running; stopping it first." -ForegroundColor Yellow
  [void](Stop-DZ Island)
}

Write-Host "Launching $exePath" -ForegroundColor Cyan
$launched = Start-Process -FilePath $exePath -PassThru
Write-Host "Waiting ${WarmupSeconds}s for startup to settle..." -ForegroundColor DarkGray
Start-Sleep -Seconds $WarmupSeconds

if ($launched.HasExited) {
  Write-Host "dz-island exited during startup (exit code $($launched.ExitCode))." -ForegroundColor Red
  Restore-NativeTaskbar
  return
}

# Note: not `$samples` -- PowerShell variable names are case-insensitive, so it
# would be the same variable as the [int]$Samples parameter.
$collected = @()
for ($i = 0; $i -lt $Samples; $i++) {
  $sample = Get-DzIslandProcessTree
  if ($null -eq $sample) { Write-Host "dz-island exited while sampling." -ForegroundColor Red; break }
  $collected += $sample
  if ($i -lt $Samples - 1) { Start-Sleep -Seconds $IntervalSeconds }
}

if ($collected.Count -eq 0) {
  if (-not $KeepRunning) { Restore-NativeTaskbar }
  return
}

$last = $collected[-1]
$ws = $collected | Measure-Object PrivateWS -Average -Minimum -Maximum
$commit = $collected | Measure-Object PrivateBytes -Average -Minimum -Maximum

Write-Host ""
Write-Host "=== DZ Island memory report ===" -ForegroundColor Cyan
Show-DzIslandMemory -Sample $last -Detail:$Detail
Write-Host ""
Write-Host ("  private WS  avg : {0}  [min {1} / max {2}]" -f (Format-MemMB $ws.Average), (Format-MemMB $ws.Minimum), (Format-MemMB $ws.Maximum)) -ForegroundColor Green
Write-Host ("  commit      avg : {0}  [min {1} / max {2}]" -f (Format-MemMB $commit.Average), (Format-MemMB $commit.Minimum), (Format-MemMB $commit.Maximum))

if (-not $KeepRunning) {
  Write-Host ""
  Write-Host "Shutting down through the app's close path..." -ForegroundColor DarkGray
  [void](Stop-DZ Island)
}
