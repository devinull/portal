[CmdletBinding()]
param(
    [string]$InstallerPath,
    [string]$ExpectedVersion = '0.0.1',
    [switch]$TestPathRegistration
)

$ErrorActionPreference = 'Stop'
$workspaceRoot = (Resolve-Path (Join-Path $PSScriptRoot '..\..')).Path
if (-not $InstallerPath) {
    $InstallerPath = Join-Path $workspaceRoot "dist\PortalSetup-$ExpectedVersion-x86_64.exe"
}
$installer = (Resolve-Path -LiteralPath $InstallerPath).Path
$testRoot = [System.IO.Path]::GetFullPath((Join-Path $workspaceRoot 'target\installer-smoke'))
$targetRoot = [System.IO.Path]::GetFullPath((Join-Path $workspaceRoot 'target'))
if (-not $testRoot.StartsWith($targetRoot, [StringComparison]::OrdinalIgnoreCase)) {
    throw "Refusing to use unexpected smoke-test path: $testRoot"
}
if (Test-Path -LiteralPath $testRoot) {
    Remove-Item -LiteralPath $testRoot -Recurse -Force
}
New-Item -ItemType Directory -Force -Path $testRoot | Out-Null

$logPath = Join-Path $testRoot 'install.log'
$originalUserPath = [Environment]::GetEnvironmentVariable('Path', 'User')
$taskSelection = if ($TestPathRegistration) { 'addtopath,!startmenu' } else { '!addtopath,!startmenu' }
$arguments = @(
    '/VERYSILENT',
    '/SUPPRESSMSGBOXES',
    '/NORESTART',
    '/CURRENTUSER',
    "/MERGETASKS=`"$taskSelection`"",
    "/DIR=`"$testRoot`"",
    "/LOG=`"$logPath`""
)
$process = Start-Process -FilePath $installer -ArgumentList $arguments -Wait -PassThru
if ($process.ExitCode -ne 0) { throw "Installer exited with code $($process.ExitCode). See $logPath" }

$installedBinary = Join-Path $testRoot 'portal.exe'
if (-not (Test-Path -LiteralPath $installedBinary)) { throw 'Installer did not create portal.exe.' }
$reportedVersion = & $installedBinary --version
if ($LASTEXITCODE -ne 0) { throw 'Installed portal.exe could not run.' }
if ($reportedVersion -notmatch [regex]::Escape($ExpectedVersion)) {
    throw "Expected version $ExpectedVersion, received: $reportedVersion"
}
if ($TestPathRegistration) {
    $installedPath = [Environment]::GetEnvironmentVariable('Path', 'User')
    $pathEntries = @($installedPath -split ';' | ForEach-Object { $_.TrimEnd('\') })
    if ($pathEntries -notcontains $testRoot.TrimEnd('\')) {
        throw 'Current-user installer did not add the selected directory to the user PATH.'
    }
}

$uninstaller = Join-Path $testRoot 'unins000.exe'
if (-not (Test-Path -LiteralPath $uninstaller)) { throw 'Installer did not create an uninstaller.' }
$uninstall = Start-Process -FilePath $uninstaller -ArgumentList '/VERYSILENT', '/SUPPRESSMSGBOXES', '/NORESTART' -Wait -PassThru
if ($uninstall.ExitCode -ne 0) { throw "Uninstaller exited with code $($uninstall.ExitCode)." }
if (Test-Path -LiteralPath $installedBinary) { throw 'Uninstaller left portal.exe behind.' }
if ($TestPathRegistration) {
    $uninstalledPath = [Environment]::GetEnvironmentVariable('Path', 'User')
    $remainingEntries = @($uninstalledPath -split ';' | ForEach-Object { $_.TrimEnd('\') })
    if ($remainingEntries -contains $testRoot.TrimEnd('\')) {
        throw 'Current-user uninstaller left the smoke-test directory on the user PATH.'
    }
    foreach ($entry in @($originalUserPath -split ';' | Where-Object { $_ })) {
        if ($remainingEntries -notcontains $entry.TrimEnd('\')) {
            throw "Uninstaller removed an unrelated PATH entry: $entry"
        }
    }
}

Write-Output "Installer smoke test passed for Portal $ExpectedVersion"
