[CmdletBinding()]
param(
    [Parameter(Mandatory)]
    [string[]]$Path,
    [string]$CertificateBase64 = $env:WINDOWS_CERTIFICATE_PFX,
    [string]$CertificatePassword = $env:WINDOWS_CERTIFICATE_PASSWORD
)

$ErrorActionPreference = 'Stop'
if (-not $CertificateBase64 -or -not $CertificatePassword) {
    throw 'Windows signing certificate and password are required.'
}

$signTool = Get-ChildItem 'C:\Program Files (x86)\Windows Kits\10\bin' `
    -Filter signtool.exe -File -Recurse | `
    Where-Object FullName -Match '\\x64\\signtool\.exe$' | `
    Sort-Object FullName -Descending | `
    Select-Object -First 1 -ExpandProperty FullName
if (-not $signTool) { throw 'signtool.exe was not found.' }

$temporaryPfx = Join-Path ([System.IO.Path]::GetTempPath()) "portal-signing-$([guid]::NewGuid()).pfx"
try {
    [System.IO.File]::WriteAllBytes(
        $temporaryPfx,
        [Convert]::FromBase64String($CertificateBase64)
    )
    foreach ($file in $Path) {
        $resolvedFile = (Resolve-Path -LiteralPath $file).Path
        & $signTool sign /fd SHA256 /td SHA256 /tr http://timestamp.digicert.com `
            /f $temporaryPfx /p $CertificatePassword $resolvedFile
        if ($LASTEXITCODE -ne 0) { throw "Signing failed for $resolvedFile" }
        & $signTool verify /pa /v $resolvedFile
        if ($LASTEXITCODE -ne 0) { throw "Signature verification failed for $resolvedFile" }
    }
}
finally {
    if (Test-Path -LiteralPath $temporaryPfx) {
        Remove-Item -LiteralPath $temporaryPfx -Force
    }
}
