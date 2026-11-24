param([Parameter(Mandatory)][string]$Plan, [switch]$ValidateOnly)
$ErrorActionPreference = 'Stop'
$planPath = [IO.Path]::GetFullPath($Plan)
$stage = Split-Path -Parent $planPath
$config = Get-Content -LiteralPath $planPath -Raw -Encoding UTF8 | ConvertFrom-Json
$root = [IO.Path]::GetFullPath([string]$config.root).TrimEnd('\')
if (!(Test-Path -LiteralPath (Join-Path $root 'Nectar.exe') -PathType Leaf)) { throw 'Missing installed application.' }
$unpack = Join-Path $stage 'unpack'
$backup = Join-Path $stage 'backup'
$allowed = @('Nectar.exe','WebView2Loader.dll','LICENSE.txt','LEIA-ME.txt','Perguntar-Na-Ilha.ps1','PERGUNTAS-NA-ILHA.txt','Battery/BatteryEngine.exe','Battery/headsetcontrol.exe','Battery/LICENSE.txt','Battery/THIRD_PARTY_NOTICES.txt')
$changed = @()
$readyToRestart = $false
function Resolve-Child([string]$Base, [string]$Relative) {
    $basePath = [IO.Path]::GetFullPath($Base).TrimEnd('\')
    $target = [IO.Path]::GetFullPath((Join-Path $basePath $Relative))
    if (!$target.StartsWith($basePath + '\', [StringComparison]::OrdinalIgnoreCase)) { throw 'Package path leaves install directory.' }
    return $target
}
function Write-Result([string]$Status, [string]$Message) {
    @{status=$Status;version=$config.version;message=$Message;date=[DateTime]::UtcNow.ToString('o')} |
        ConvertTo-Json -Compress | Set-Content -LiteralPath (Join-Path $root 'data/update-result.json') -Encoding UTF8
}
try {
    Add-Type -AssemblyName System.IO.Compression.FileSystem
    $zip = [IO.Compression.ZipFile]::OpenRead((Join-Path $stage 'package.zip'))
    try {
        if ($zip.Entries.Count -gt 30) { throw 'Too many package entries.' }
        $seen = @{}
        $total = 0L
        foreach ($entry in $zip.Entries) {
            $name = $entry.FullName.Replace('\','/')
            if ($name -eq 'Battery/') { continue }
            if ($allowed -notcontains $name -or $seen.ContainsKey($name)) { throw "Unexpected or duplicate package entry: $name" }
            $seen[$name] = $true
            $total += $entry.Length
            if ($entry.Length -gt 150MB -or $total -gt 250MB) { throw 'Unpacked package is too large.' }
            $dest = Resolve-Child $unpack $name
            [void][IO.Directory]::CreateDirectory((Split-Path $dest))
            [IO.Compression.ZipFileExtensions]::ExtractToFile($entry,$dest,$false)
        }
        if (!$seen.ContainsKey('Nectar.exe')) { throw 'Missing new application.' }
    } finally { $zip.Dispose() }
    $binary = [IO.File]::OpenRead((Join-Path $unpack 'Nectar.exe'))
    try { if ($binary.ReadByte() -ne 77 -or $binary.ReadByte() -ne 90) { throw 'Invalid Windows executable.' } } finally { $binary.Dispose() }
    if ($ValidateOnly) { return }
    $parentProcess = Get-Process -Id ([int]$config.parent) -ErrorAction SilentlyContinue
    if ($parentProcess) { Wait-Process -Id $parentProcess.Id -Timeout 60 -ErrorAction Stop }
    $readyToRestart = $true
    Start-Sleep -Seconds 2
    # Only stop the battery companion belonging to this installation.
    $engine = Resolve-Child $root 'Battery/BatteryEngine.exe'
    Get-CimInstance Win32_Process | Where-Object { $_.ExecutablePath -eq $engine } | ForEach-Object { Stop-Process -Id $_.ProcessId -Force -ErrorAction Stop }
    foreach ($name in $seen.Keys) {
        $target = Resolve-Child $root $name
        $old = Resolve-Child $backup $name
        $exists = Test-Path -LiteralPath $target -PathType Leaf
        if ($exists) { [void][IO.Directory]::CreateDirectory((Split-Path $old)); Copy-Item -LiteralPath $target -Destination $old -Force }
        $changed += @{target=$target;backup=$old;existed=$exists}
        [void][IO.Directory]::CreateDirectory((Split-Path $target))
        Copy-Item -LiteralPath (Resolve-Child $unpack $name) -Destination $target -Force
    }
    Write-Result 'installed' 'Update installed; data and settings preserved.'
} catch {
    $failure = $_.Exception.Message
    foreach ($item in $changed) {
        try {
            if ($item.existed) { Copy-Item -LiteralPath $item.backup -Destination $item.target -Force }
            elseif (Test-Path -LiteralPath $item.target -PathType Leaf) {
                # Targets were resolved and checked against the install root above.
                if (!$item.target.StartsWith($root + '\',[StringComparison]::OrdinalIgnoreCase)) { throw 'Invalid rollback path.' }
                Remove-Item -LiteralPath $item.target -Force
            }
        } catch { $failure += '; rollback: ' + $_.Exception.Message }
    }
    try { Write-Result 'failed' $failure } catch {}
    $failure | Set-Content -LiteralPath (Join-Path $stage 'error.txt') -Encoding UTF8
    # The app may already have exited while validation was running.
    if (!(Get-Process -Id ([int]$config.parent) -ErrorAction SilentlyContinue)) { $readyToRestart = $true }
} finally {
    if ($readyToRestart) {
        Start-Process -FilePath (Resolve-Child $root 'Nectar.exe') -WorkingDirectory $root -WindowStyle Hidden
    }
}
