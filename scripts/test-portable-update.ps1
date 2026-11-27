param([string]$Helper = (Join-Path $PSScriptRoot '../src-tauri/target/release/dz-update-helper.exe'))
$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.IO.Compression.FileSystem
$testRoot = Join-Path ([IO.Path]::GetTempPath()) ('dz-updater-tests-' + [guid]::NewGuid().ToString('N'))

$cases = @(
    @{name='valid';entries=@('DZ-Island.exe','WebView2Loader.dll');fail=$false},
    @{name='traversal';entries=@('DZ-Island.exe','../outside.txt');fail=$true},
    @{name='settings';entries=@('DZ-Island.exe','data/settings.json');fail=$true},
    @{name='duplicate';entries=@('DZ-Island.exe','DZ-ISLAND.EXE');fail=$true},
    @{name='missing-exe';entries=@('LICENSE.txt');fail=$true},
    @{name='unexpected-script';entries=@('DZ-Island.exe','run.bat');fail=$true},
    @{name='bad-digest';entries=@('DZ-Island.exe');fail=$true}
)
foreach ($case in $cases) {
    $stage = Join-Path $testRoot $case.name
    $root = Join-Path $stage 'installed'
    [void][IO.Directory]::CreateDirectory((Join-Path $root 'data'))
    [IO.File]::WriteAllBytes((Join-Path $root 'DZ-Island.exe'),[byte[]](77,90,0))
    [IO.File]::WriteAllText((Join-Path $root 'data/settings.json'),'{"preserve":true}')
    $archive = [IO.Compression.ZipFile]::Open((Join-Path $stage 'package.zip'),[IO.Compression.ZipArchiveMode]::Create)
    try {
        foreach ($name in $case.entries) {
            $entry=$archive.CreateEntry($name)
            $stream=$entry.Open()
            try { $bytes=[byte[]](77,90,1); $stream.Write($bytes,0,$bytes.Length) } finally { $stream.Dispose() }
        }
    } finally { $archive.Dispose() }
    $plan = Join-Path $stage 'plan.json'
    $digest=(Get-FileHash -LiteralPath (Join-Path $stage 'package.zip') -Algorithm SHA256).Hash.ToLowerInvariant()
    if ($case.name -eq 'bad-digest') {$digest='0'*64}
    @{root=$root;parent=$PID;version='1.11.1';digest=$digest} | ConvertTo-Json | Set-Content -LiteralPath $plan -Encoding UTF8
    $process = Start-Process -FilePath $Helper -ArgumentList @(('"' + $plan + '"'),'--validate-only') -Wait -PassThru -WindowStyle Hidden
    $failed=Test-Path -LiteralPath (Join-Path $stage 'error.txt')
    if ($failed -ne $case.fail) { throw "Unexpected validation result: $($case.name)" }
    if ([IO.File]::ReadAllText((Join-Path $root 'data/settings.json')) -ne '{"preserve":true}') { throw 'Settings modified during validation.' }
    if ([IO.File]::ReadAllBytes((Join-Path $root 'DZ-Island.exe'))[2] -ne 0) { throw 'Application changed during validation.' }
    Write-Output "PASS $($case.name)"
}
Write-Output '7 archive boundary tests passed.'

# Exercise replacement, restart and rollback with harmless mock applications.
$compiler = Join-Path $env:SystemRoot 'Microsoft.NET/Framework64/v4.0.30319/csc.exe'
$mockSource = Join-Path $testRoot 'mock.cs'
$template = 'using System; using System.IO; using System.Threading; class Mock { static void Main(string[] args) { string root = AppDomain.CurrentDomain.BaseDirectory; if (args.Length != 0) { File.WriteAllText(Path.Combine(root,"background.ready"),"ready"); Thread.Sleep(45000); return; } File.WriteAllText(Path.Combine(root,"restarted.txt"),"MARKER"); } }'
$oldExe = Join-Path $testRoot 'old.exe'
$newExe = Join-Path $testRoot 'new.exe'
foreach ($item in @(@{marker='old';path=$oldExe},@{marker='new';path=$newExe})) {
    [IO.File]::WriteAllText($mockSource,$template.Replace('MARKER',$item.marker))
    & $compiler /nologo /target:winexe "/out:$($item.path)" $mockSource
    if ($LASTEXITCODE -ne 0) { throw 'Mock application compilation failed.' }
}
foreach ($scenario in @('replace','legacy','running-background','rollback')) {
    $stage = Join-Path $testRoot $scenario
    $root = Join-Path $stage 'installed'
    [void][IO.Directory]::CreateDirectory((Join-Path $root 'data'))
    $app = if ($scenario -eq 'legacy') { 'Nectar.exe' } else { 'DZ-Island.exe' }
    Copy-Item -LiteralPath $oldExe -Destination (Join-Path $root $app)
    [IO.File]::WriteAllText((Join-Path $root 'data/settings.json'),'preserved')
    [IO.File]::WriteAllText((Join-Path $root 'LICENSE.txt'),'old-license')
    [IO.File]::WriteAllText((Join-Path $root 'WebView2Loader.dll'),'old-loader')
    $archive = [IO.Compression.ZipFile]::Open((Join-Path $stage 'package.zip'),[IO.Compression.ZipArchiveMode]::Create)
    try {
        [void][IO.Compression.ZipFileExtensions]::CreateEntryFromFile($archive,$newExe,$app)
        foreach ($name in @('LICENSE.txt','WebView2Loader.dll')) {
            $entry=$archive.CreateEntry($name); $writer=[IO.StreamWriter]::new($entry.Open())
            try { $writer.Write('new-content') } finally { $writer.Dispose() }
        }
    } finally { $archive.Dispose() }
    $plan=Join-Path $stage 'plan.json'
    @{root=$root;parent=2147483647;version='1.11.1';digest=(Get-FileHash -LiteralPath (Join-Path $stage 'package.zip') -Algorithm SHA256).Hash.ToLowerInvariant()} | ConvertTo-Json | Set-Content -LiteralPath $plan -Encoding UTF8
    $lock=$null
    $background=$null
    try {
        if ($scenario -eq 'running-background') {
            $background=Start-Process -FilePath (Join-Path $root 'DZ-Island.exe') -ArgumentList '--background' -PassThru -WindowStyle Hidden
            $deadline=(Get-Date).AddSeconds(10)
            while (-not (Test-Path (Join-Path $root 'background.ready')) -and (Get-Date) -lt $deadline) {
                Start-Sleep -Milliseconds 100
            }
            if (-not (Test-Path (Join-Path $root 'background.ready'))) { throw 'Background mock did not start.' }
        }
        if ($scenario -eq 'rollback') { $lock=[IO.File]::Open((Join-Path $root 'WebView2Loader.dll'),[IO.FileMode]::Open,[IO.FileAccess]::ReadWrite,[IO.FileShare]::None) }
        $process = Start-Process -FilePath $Helper -ArgumentList @('"' + $plan + '"') -Wait -PassThru -WindowStyle Hidden
        if ($background -and $background.HasExited) { throw 'The updater stopped the background process.' }
    } finally {
        if ($lock) { $lock.Dispose() }
        if ($background -and -not $background.HasExited) { $background.Kill() }
    }
    Start-Sleep -Milliseconds 800
    $result=Get-Content -LiteralPath (Join-Path $root 'data/update-result.json') -Raw | ConvertFrom-Json
    $expected=if($scenario -eq 'rollback'){'failed'}else{'installed'}
    $marker=if($scenario -eq 'rollback'){'old'}else{'new'}
    if ($result.status -ne $expected) { throw "Unexpected $scenario result: $($result.status)" }
    if ([IO.File]::ReadAllText((Join-Path $root 'restarted.txt')) -ne $marker) { throw "Incorrect application restarted after $scenario" }
    if ([IO.File]::ReadAllText((Join-Path $root 'data/settings.json')) -ne 'preserved') { throw 'Settings were lost.' }
    if ($scenario -eq 'rollback' -and [IO.File]::ReadAllText((Join-Path $root 'LICENSE.txt')) -ne 'old-license') { throw 'Rollback did not restore all files.' }
    Write-Output "PASS $scenario and restart"
}
Write-Output '4 install/restart/rollback tests passed.'
