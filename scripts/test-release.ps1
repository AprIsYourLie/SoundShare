$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest
Add-Type -AssemblyName System.IO.Compression.FileSystem

$verifyScript = Join-Path $PSScriptRoot "verify-release.ps1"
$temporaryRoot = Join-Path ([IO.Path]::GetTempPath()) ("soundshare-test-" + [guid]::NewGuid())
$package = Join-Path $temporaryRoot "release with spaces"
$payload = @("SoundShare.exe", "SoundShare-CLI.exe", "README.md", "LICENSE")
$downloads = @("SoundShare.exe", "SoundShare-Windows-x64.zip")

function Write-Checksums {
    $lines = foreach ($name in $downloads) {
        "{0}  {1}" -f (Get-FileHash -LiteralPath (Join-Path $package $name) -Algorithm SHA256).Hash.ToLowerInvariant(), $name
    }
    Set-Content -LiteralPath (Join-Path $package "SHA256SUMS.txt") -Value $lines -Encoding ASCII
}

function Reset-Package {
    if (Test-Path -LiteralPath $package) { Remove-Item -LiteralPath $package -Recurse -Force }
    New-Item -ItemType Directory -Path $package -Force | Out-Null
    foreach ($name in $payload) {
        Set-Content -LiteralPath (Join-Path $package $name) -Value "fixture: $name" -Encoding ASCII
    }
    Compress-Archive -LiteralPath @($payload | ForEach-Object { Join-Path $package $_ }) -DestinationPath (Join-Path $package $downloads[1])
    Write-Checksums
}

function Assert-Rejected([string]$Name, [scriptblock]$Mutate, [string]$Message) {
    Reset-Package
    & $Mutate
    $rejected = $false
    try { & $verifyScript -ReleaseDirectory $package }
    catch {
        if ($_.Exception.Message -notlike "*$Message*") { throw "${Name}: unexpected failure: $_" }
        $rejected = $true
    }
    if (-not $rejected) { throw "${Name}: invalid package was accepted" }
    Write-Host "PASS: $Name"
}

try {
    Reset-Package
    & $verifyScript -ReleaseDirectory $package
    Write-Host "PASS: valid package (path with spaces)"
    Assert-Rejected "missing CLI" { Remove-Item (Join-Path $package "SoundShare-CLI.exe") } "Missing release file"
    Assert-Rejected "empty executable" { [IO.File]::WriteAllBytes((Join-Path $package "SoundShare.exe"), [byte[]]@()) } "Empty release file"
    Assert-Rejected "changed executable" { Add-Content (Join-Path $package "SoundShare.exe") "changed" } "SHA-256 mismatch"
    Assert-Rejected "changed archive" { Add-Content (Join-Path $package $downloads[1]) "changed" } "SHA-256 mismatch"
    Assert-Rejected "missing checksum file" { Remove-Item (Join-Path $package "SHA256SUMS.txt") } "Missing release file"
    Assert-Rejected "malformed checksum" { Set-Content (Join-Path $package "SHA256SUMS.txt") "invalid" } "Invalid checksum line"
    Assert-Rejected "missing checksum entry" {
        $line = (Get-Content (Join-Path $package "SHA256SUMS.txt"))[0]
        Set-Content (Join-Path $package "SHA256SUMS.txt") $line
    } "Checksum file must contain"
    Assert-Rejected "duplicate checksum" {
        $line = (Get-Content (Join-Path $package "SHA256SUMS.txt"))[0]
        Add-Content (Join-Path $package "SHA256SUMS.txt") $line
    } "Duplicate checksum"
    Assert-Rejected "unexpected checksum path" {
        Add-Content (Join-Path $package "SHA256SUMS.txt") (("0" * 64) + "  ../outside.exe")
    } "Unexpected checksum file"
    Assert-Rejected "archive payload differs" {
        Add-Content (Join-Path $package "README.md") "changed"
    } "ZIP content mismatch"
    Assert-Rejected "same-size archive payload differs" {
        $path = Join-Path $package "README.md"
        $bytes = [IO.File]::ReadAllBytes($path)
        $bytes[0] = $bytes[0] -bxor 1
        [IO.File]::WriteAllBytes($path, $bytes)
    } "ZIP content mismatch: README.md (SHA-256)"
    Assert-Rejected "extra settings in archive" {
        $zip = [IO.Compression.ZipFile]::Open((Join-Path $package $downloads[1]), 'Update')
        try { $null = $zip.CreateEntry("settings.json") } finally { $zip.Dispose() }
        Write-Checksums
    } "Unexpected ZIP entry"
    Assert-Rejected "missing archive entry" {
        $zip = [IO.Compression.ZipFile]::Open((Join-Path $package $downloads[1]), 'Update')
        try { $zip.GetEntry("LICENSE").Delete() } finally { $zip.Dispose() }
        Write-Checksums
    } "ZIP must contain"
    Assert-Rejected "duplicate archive entry" {
        $zip = [IO.Compression.ZipFile]::Open((Join-Path $package $downloads[1]), 'Update')
        try { $null = $zip.CreateEntry("LICENSE") } finally { $zip.Dispose() }
        Write-Checksums
    } "Duplicate ZIP entry"
    Assert-Rejected "invalid archive with matching checksum" {
        Set-Content (Join-Path $package $downloads[1]) "not a zip"
        Write-Checksums
    } "Invalid ZIP archive"
    Write-Host "All 16 release integrity tests passed."
}
finally {
    if (Test-Path -LiteralPath $temporaryRoot) { Remove-Item -LiteralPath $temporaryRoot -Recurse -Force }
}
