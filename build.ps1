param(
    [ValidateSet("build", "test", "run")]
    [string]$Action = "build",
    [switch]$Release,
    [Parameter(ValueFromRemainingArguments = $true)]
    [string[]]$ProgramArguments
)

$ErrorActionPreference = "Stop"

if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) {
    throw "Cargo was not found. Install Rust with rustup, then open a new terminal."
}

if (-not (Get-Command cmake -ErrorAction SilentlyContinue)) {
    $vswhere = "${env:ProgramFiles(x86)}\Microsoft Visual Studio\Installer\vswhere.exe"
    if (Test-Path -LiteralPath $vswhere) {
        $installations = & $vswhere -products * -property installationPath
        foreach ($installation in $installations) {
            $candidate = Join-Path $installation "Common7\IDE\CommonExtensions\Microsoft\CMake\CMake\bin\cmake.exe"
            if (Test-Path -LiteralPath $candidate) {
                $env:CMAKE = $candidate
                break
            }
        }
    }
}

if (-not $env:CMAKE -and -not (Get-Command cmake -ErrorAction SilentlyContinue)) {
    throw "CMake was not found. Add the CMake component in Visual Studio Build Tools."
}

$cargoArguments = @($Action)
if ($Release) {
    $cargoArguments += @("--profile", "dist")
}
if ($Action -eq "run" -and $ProgramArguments.Count -gt 0) {
    $cargoArguments += "--"
    $cargoArguments += $ProgramArguments
}

Push-Location $PSScriptRoot
try {
    & cargo @cargoArguments
    $cargoExitCode = $LASTEXITCODE
}
finally {
    Pop-Location
}
if ($cargoExitCode -ne 0) {
    exit $cargoExitCode
}

if ($Action -eq "build" -and $Release) {
    $releaseDirectory = Join-Path $PSScriptRoot "release"
    New-Item -ItemType Directory -Path $releaseDirectory -Force | Out-Null
    Copy-Item -LiteralPath (Join-Path $PSScriptRoot "target\dist\soundshare-gui.exe") -Destination (Join-Path $releaseDirectory "SoundShare.exe") -Force
    Copy-Item -LiteralPath (Join-Path $PSScriptRoot "target\dist\soundshare.exe") -Destination (Join-Path $releaseDirectory "SoundShare-CLI.exe") -Force
    Copy-Item -LiteralPath (Join-Path $PSScriptRoot "README.md") -Destination (Join-Path $releaseDirectory "README.md") -Force
    Copy-Item -LiteralPath (Join-Path $PSScriptRoot "LICENSE") -Destination (Join-Path $releaseDirectory "LICENSE") -Force

    $archivePath = Join-Path $releaseDirectory "SoundShare-Windows-x64.zip"
    Compress-Archive -LiteralPath @(
        (Join-Path $releaseDirectory "SoundShare.exe"),
        (Join-Path $releaseDirectory "SoundShare-CLI.exe"),
        (Join-Path $releaseDirectory "README.md"),
        (Join-Path $releaseDirectory "LICENSE")
    ) -DestinationPath $archivePath -Force
    $checksumLines = foreach ($name in @("SoundShare.exe", "SoundShare-Windows-x64.zip")) {
        $hash = (Get-FileHash -LiteralPath (Join-Path $releaseDirectory $name) -Algorithm SHA256).Hash.ToLowerInvariant()
        "{0}  {1}" -f $hash, $name
    }
    Set-Content -LiteralPath (Join-Path $releaseDirectory "SHA256SUMS.txt") -Value $checksumLines -Encoding ASCII
    & (Join-Path $PSScriptRoot "scripts\verify-release.ps1") -ReleaseDirectory $releaseDirectory
    Write-Host "Package created: $archivePath"
}

exit 0
