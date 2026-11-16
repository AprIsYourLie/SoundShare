param(
    [string]$ReleaseDirectory = (Join-Path $PSScriptRoot "..\release")
)

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest
Add-Type -AssemblyName System.IO.Compression.FileSystem

$payload = @("SoundShare.exe", "SoundShare-CLI.exe", "README.md", "LICENSE")
$downloads = @("SoundShare.exe", "SoundShare-Windows-x64.zip")
foreach ($name in ($payload + @("SoundShare-Windows-x64.zip", "SHA256SUMS.txt"))) {
    $path = Join-Path $ReleaseDirectory $name
    if (-not (Test-Path -LiteralPath $path -PathType Leaf)) { throw "Missing release file: $name" }
    if ((Get-Item -LiteralPath $path).Length -eq 0) { throw "Empty release file: $name" }
}

# Validate, never regenerate: stale hashes must fail even when files are present.
$seen = @{}
foreach ($line in (Get-Content -LiteralPath (Join-Path $ReleaseDirectory "SHA256SUMS.txt"))) {
    if ($line -cnotmatch '^([0-9a-fA-F]{64})  (.+)$') { throw "Invalid checksum line: $line" }
    $expectedHash = $Matches[1]
    $name = $Matches[2]
    if ($downloads -cnotcontains $name) { throw "Unexpected checksum file: $name" }
    if ($seen.ContainsKey($name)) { throw "Duplicate checksum: $name" }
    $seen[$name] = $true
    $actualHash = (Get-FileHash -LiteralPath (Join-Path $ReleaseDirectory $name) -Algorithm SHA256).Hash
    if ($actualHash -ne $expectedHash) { throw "SHA-256 mismatch: $name" }
}
if ($seen.Count -ne $downloads.Count) { throw "Checksum file must contain exactly the EXE and ZIP hashes." }

# Read entries without extracting untrusted paths. An exact allowlist rejects
# nested paths, personal settings/identity files, directories and duplicates.
try {
    $archive = [IO.Compression.ZipFile]::OpenRead((Resolve-Path -LiteralPath (Join-Path $ReleaseDirectory $downloads[1])).Path)
}
catch { throw "Invalid ZIP archive: $_" }
try {
    $entries = @{}
    foreach ($entry in $archive.Entries) {
        if ($payload -cnotcontains $entry.FullName) { throw "Unexpected ZIP entry: $($entry.FullName)" }
        if ($entries.ContainsKey($entry.FullName)) { throw "Duplicate ZIP entry: $($entry.FullName)" }
        $entries[$entry.FullName] = $entry
    }
    if ($entries.Count -ne $payload.Count) { throw "ZIP must contain exactly the four release payload files." }
    foreach ($name in $payload) {
        $localPath = Join-Path $ReleaseDirectory $name
        if ($entries[$name].Length -ne (Get-Item -LiteralPath $localPath).Length) {
            throw "ZIP content mismatch: $name (length)"
        }
        $stream = $entries[$name].Open()
        $sha256 = [Security.Cryptography.SHA256]::Create()
        try {
            $entryHash = [BitConverter]::ToString($sha256.ComputeHash($stream)).Replace("-", "")
        }
        finally { $stream.Dispose(); $sha256.Dispose() }
        if ($entryHash -ne (Get-FileHash -LiteralPath $localPath -Algorithm SHA256).Hash) {
            throw "ZIP content mismatch: $name (SHA-256)"
        }
    }
}
finally { $archive.Dispose() }

Write-Host "Release integrity verified: four ZIP entries, matching payloads, EXE and ZIP SHA-256."
