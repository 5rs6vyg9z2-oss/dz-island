$ErrorActionPreference = 'Stop'
$root = Split-Path $PSScriptRoot -Parent
$build = Join-Path $root 'build'
New-Item -ItemType Directory -Force $build | Out-Null
$lines = foreach ($file in Get-ChildItem -LiteralPath (Join-Path $root 'lang') -Filter '*.txt' | Sort-Object Name) {
    "@file $($file.BaseName)"
    [IO.File]::ReadAllLines($file.FullName) | Where-Object {
        $_ -notmatch 'krisgfx|GitHub|Support the project\.\.\.'
    }
    ''
}
[IO.File]::WriteAllLines((Join-Path $build 'languages.txt'), [string[]]$lines, [Text.UTF8Encoding]::new($false))
