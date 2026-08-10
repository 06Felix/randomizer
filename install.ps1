param(
    [string]$Version = "latest"
)

$ErrorActionPreference = "Stop"

$Repo = "06Felix/randomizer"
$BinaryName = "randomizer.exe"

if (-not [System.Runtime.InteropServices.RuntimeInformation]::IsOSPlatform(
    [System.Runtime.InteropServices.OSPlatform]::Windows
)) {
    throw "Unsupported operating system. This installer supports Windows x86-64 only."
}

$Arch = [System.Runtime.InteropServices.RuntimeInformation]::OSArchitecture
if ($Arch -ne [System.Runtime.InteropServices.Architecture]::X64) {
    throw "Unsupported Windows architecture: $Arch. Only Windows x86-64 is supported."
}

$Target = "x86_64-pc-windows-msvc"
$Ext = "zip"

# GitHub headers
$Headers = @{
    "Accept" = "application/vnd.github+json"
    "User-Agent" = "randomizer-installer"
}

if ($Version -eq "latest") {
    $Release = Invoke-RestMethod `
        -Uri "https://api.github.com/repos/$Repo/releases/latest" `
        -Headers $Headers

    $Version = $Release.tag_name

    if (-not $Version) {
        throw "Failed to fetch latest version"
    }
}

$File = "randomizer-$Version-$Target.$Ext"
$Url = "https://github.com/$Repo/releases/download/$Version/$File"

Write-Host "Downloading $Url..."

$TempDir = Join-Path ([System.IO.Path]::GetTempPath()) ([System.Guid]::NewGuid())
New-Item -ItemType Directory -Path $TempDir | Out-Null
Set-Location $TempDir

try {
    Invoke-WebRequest $Url -OutFile $File -ErrorAction Stop
} catch {
    throw "Download failed: $Url"
}

Write-Host "Extracting..."
Expand-Archive $File -Force

$BinPath = Get-ChildItem -Recurse -Filter $BinaryName | Select-Object -First 1

if (-not $BinPath) {
    throw "Binary not found in archive"
}

$InstallDir = "$env:LOCALAPPDATA\Programs\randomizer"
New-Item -ItemType Directory -Force -Path $InstallDir | Out-Null

Move-Item $BinPath.FullName "$InstallDir\$BinaryName" -Force

Write-Host "Installed to $InstallDir"

# PATH fix
$CurrentPath = [Environment]::GetEnvironmentVariable("PATH", "User")
$Paths = $CurrentPath -split ';'

if ($Paths -notcontains $InstallDir) {
    [Environment]::SetEnvironmentVariable("PATH", "$CurrentPath;$InstallDir", "User")
    Write-Host "Added to PATH. Restart terminal."
}

Write-Host "✅ Done!"
