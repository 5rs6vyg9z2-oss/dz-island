param(
    [Parameter(Mandatory)][string]$Setup,
    [Parameter(Mandatory)][string]$Payload,
    [Parameter(Mandatory)][string]$Version
)
$ErrorActionPreference = 'Stop'
$root = Join-Path ([IO.Path]::GetTempPath()) ('dz-island-setup-test-' + [guid]::NewGuid())
$menu = Join-Path ([Environment]::GetFolderPath('Programs')) 'DZ Island.lnk'
$key = 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\DZIsland'
if ((Test-Path -LiteralPath $menu) -or (Test-Path -LiteralPath $key)) {
    throw 'Run the installer test on a clean Windows account.'
}
$install = Start-Process -FilePath $Setup -ArgumentList "/S /D=$root" -WindowStyle Hidden -Wait -PassThru
if ($install.ExitCode -ne 0) { throw "Installer exited with code $($install.ExitCode)." }
$registration = Get-ItemProperty -LiteralPath $key
if ($registration.DisplayVersion -ne $Version -or $registration.InstallLocation -ne $root) {
    throw 'Unexpected Windows uninstall registration.'
}
$link = (New-Object -ComObject WScript.Shell).CreateShortcut($menu)
if ($link.TargetPath -ne (Join-Path $root 'DZ-Island.exe')) { throw 'Shortcut target mismatch.' }
foreach ($file in Get-ChildItem -LiteralPath $Payload -Recurse -File) {
    $destination = Join-Path $root ([IO.Path]::GetRelativePath($Payload, $file.FullName))
    if ((Get-FileHash -LiteralPath $destination).Hash -ne (Get-FileHash -LiteralPath $file.FullName).Hash) {
        throw "Payload mismatch: $($file.Name)"
    }
}
$data = Join-Path $root 'data'
New-Item -ItemType Directory -Path $data | Out-Null
'preserved' | Set-Content -LiteralPath (Join-Path $data 'settings.json')
$uninstall = Start-Process -FilePath (Join-Path $root 'Uninstall.exe') -ArgumentList "/S _?=$root" -WindowStyle Hidden -Wait -PassThru
if ($uninstall.ExitCode -ne 0 -or (Test-Path -LiteralPath $menu) -or (Test-Path -LiteralPath $key) -or
    (Test-Path -LiteralPath (Join-Path $root 'DZ-Island.exe')) -or
    (Get-Content -LiteralPath (Join-Path $data 'settings.json')) -ne 'preserved') {
    throw 'Uninstall or profile preservation check failed.'
}
'Installation, payload, shortcut and profile preservation verified.'
