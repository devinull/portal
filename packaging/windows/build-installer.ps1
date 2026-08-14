[CmdletBinding()]
param(
    [string]$Version,
    [string]$BinaryPath,
    [string]$OutputDirectory,
    [switch]$SkipBuild
)

$ErrorActionPreference = 'Stop'
$workspaceRoot = (Resolve-Path (Join-Path $PSScriptRoot '..\..')).Path

if (-not $Version) {
    $metadata = cargo metadata --no-deps --format-version 1 | ConvertFrom-Json
    $Version = ($metadata.packages | Where-Object name -eq 'portal-cli').version
}

if (-not $SkipBuild) {
    cargo build --workspace --release --locked
    if ($LASTEXITCODE -ne 0) { throw 'Release build failed.' }
}

if (-not $BinaryPath) {
    $BinaryPath = Join-Path $workspaceRoot 'target\release\portal.exe'
}
if (-not $OutputDirectory) {
    $OutputDirectory = Join-Path $workspaceRoot 'dist'
}

$resolvedBinary = (Resolve-Path -LiteralPath $BinaryPath).Path
$resolvedOutput = [System.IO.Path]::GetFullPath($OutputDirectory)
New-Item -ItemType Directory -Force -Path $resolvedOutput | Out-Null

$compilerCandidates = @(
    (Join-Path $env:LOCALAPPDATA 'Programs\Inno Setup 6\ISCC.exe'),
    'C:\Program Files (x86)\Inno Setup 6\ISCC.exe',
    'C:\Program Files\Inno Setup 6\ISCC.exe'
)
$compiler = $compilerCandidates | Where-Object { Test-Path -LiteralPath $_ } | Select-Object -First 1
if (-not $compiler) {
    throw 'Inno Setup 6.6 or newer is required. Install JRSoftware.InnoSetup with WinGet.'
}

$installerScript = Join-Path $PSScriptRoot 'portal.iss'
& $compiler "/DPortalVersion=$Version" "/DPortalBinary=$resolvedBinary" "/DPortalOutputDir=$resolvedOutput" $installerScript
if ($LASTEXITCODE -ne 0) { throw 'Inno Setup compilation failed.' }

$staging = Join-Path $workspaceRoot 'target\windows-archive'
if (Test-Path -LiteralPath $staging) {
    $resolvedStaging = (Resolve-Path -LiteralPath $staging).Path
    if (-not $resolvedStaging.StartsWith((Join-Path $workspaceRoot 'target'), [StringComparison]::OrdinalIgnoreCase)) {
        throw "Refusing to clear unexpected staging path: $resolvedStaging"
    }
    Remove-Item -LiteralPath $resolvedStaging -Recurse -Force
}
New-Item -ItemType Directory -Force -Path $staging | Out-Null
Copy-Item -LiteralPath $resolvedBinary -Destination (Join-Path $staging 'portal.exe')
Copy-Item -LiteralPath (Join-Path $workspaceRoot 'README.md') -Destination $staging
Copy-Item -LiteralPath (Join-Path $workspaceRoot 'LICENSE') -Destination $staging
Copy-Item -LiteralPath (Join-Path $workspaceRoot 'CHANGELOG.md') -Destination $staging

$archive = Join-Path $resolvedOutput "portal-$Version-x86_64-pc-windows-msvc.zip"
if (Test-Path -LiteralPath $archive) { Remove-Item -LiteralPath $archive -Force }
Compress-Archive -Path (Join-Path $staging '*') -DestinationPath $archive -CompressionLevel Optimal

Write-Output "Created Windows release assets in $resolvedOutput"
