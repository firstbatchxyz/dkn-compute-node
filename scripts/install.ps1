# Dria Node installer for Windows
# Usage: irm https://raw.githubusercontent.com/firstbatchxyz/dkn-compute-node/v2/scripts/install.ps1 | iex
$ErrorActionPreference = "Stop"

$Repo = "firstbatchxyz/dkn-compute-node"
$Binary = "dria-node"
$InstallDir = "$env:LOCALAPPDATA\dria"

Write-Host "Dria Node Installer" -ForegroundColor Cyan

# Fetch latest release
Write-Host "Fetching latest release..." -ForegroundColor Blue
try {
    $Release = Invoke-RestMethod -Uri "https://api.github.com/repos/$Repo/releases/latest"
    $Tag = $Release.tag_name
} catch {
    Write-Host "Error: Failed to fetch latest release. Check your internet connection." -ForegroundColor Red
    exit 1
}

Write-Host "Latest release: $Tag" -ForegroundColor Blue

# Download binary
$Asset = "$Binary-windows-amd64.exe"
$Url = "https://github.com/$Repo/releases/download/$Tag/$Asset"

Write-Host "Downloading $Asset..." -ForegroundColor Blue
$TmpFile = Join-Path $env:TEMP "$Binary.exe"
try {
    Invoke-WebRequest -Uri $Url -OutFile $TmpFile -UseBasicParsing
} catch {
    Write-Host "Error: Download failed. Asset may not exist: $Url" -ForegroundColor Red
    exit 1
}

# Install
if (-not (Test-Path $InstallDir)) {
    New-Item -ItemType Directory -Path $InstallDir -Force | Out-Null
}
$Dest = Join-Path $InstallDir "$Binary.exe"
Move-Item -Path $TmpFile -Destination $Dest -Force
Write-Host "Installed to $Dest" -ForegroundColor Blue

# Add to PATH if not present
$UserPath = [Environment]::GetEnvironmentVariable("PATH", "User")
if ($UserPath -notlike "*$InstallDir*") {
    [Environment]::SetEnvironmentVariable("PATH", "$InstallDir;$UserPath", "User")
    $env:PATH = "$InstallDir;$env:PATH"
    Write-Host "Added $InstallDir to user PATH." -ForegroundColor Blue
    Write-Host "Restart your terminal for PATH changes to take effect." -ForegroundColor Yellow
}

# Verify
Write-Host ""
try {
    $Version = & $Dest --version 2>&1
    Write-Host "Successfully installed $Version" -ForegroundColor Green
} catch {
    Write-Host "Installed successfully. Run '$Binary --version' to verify." -ForegroundColor Green
}
Write-Host "Run '$Binary start --help' to get started." -ForegroundColor Cyan
