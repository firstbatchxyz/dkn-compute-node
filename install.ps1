$ErrorActionPreference = "Stop"

$repo = "firstbatchxyz/dkn-compute-node"
$binary = "dria-node"

# Get latest release tag (includes pre-releases)
$releases = Invoke-RestMethod "https://api.github.com/repos/$repo/releases"
$tag = $releases[0].tag_name
if (-not $tag) {
    Write-Error "Failed to fetch latest release"
    exit 1
}

$asset = "$binary-windows-amd64.exe"
$url = "https://github.com/$repo/releases/download/$tag/$asset"

$installDir = "$env:LOCALAPPDATA\dria-node"
if (-not (Test-Path $installDir)) {
    New-Item -ItemType Directory -Path $installDir | Out-Null
}

$dest = Join-Path $installDir "$binary.exe"

Write-Host "Installing $binary $tag..."
Invoke-WebRequest -Uri $url -OutFile $dest

# Add to PATH if not already there
$userPath = [Environment]::GetEnvironmentVariable("Path", "User")
if ($userPath -notlike "*$installDir*") {
    [Environment]::SetEnvironmentVariable("Path", "$userPath;$installDir", "User")
    $env:Path = "$env:Path;$installDir"
    Write-Host "Added $installDir to PATH"
}

Write-Host "Installed $binary to $dest"
& $dest --version
