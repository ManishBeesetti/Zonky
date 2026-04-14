param(
    [switch]$CheckOnly,
    [switch]$KillExisting
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

function Fail {
    param([string]$Message)
    Write-Host ""
    Write-Host "[ERR] $Message" -ForegroundColor Red
    exit 1
}

function Require-Command {
    param(
        [string]$Name,
        [string]$InstallHint
    )

    $cmd = Get-Command $Name -ErrorAction SilentlyContinue
    if (-not $cmd) {
        Fail "$Name not found. $InstallHint"
    }
}

function Test-MsvcBuildTools {
    $vswhere = Join-Path ${env:ProgramFiles(x86)} "Microsoft Visual Studio\Installer\vswhere.exe"
    if (-not (Test-Path $vswhere)) {
        return $false
    }

    $installPath = & $vswhere -latest -products * -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath
    return -not [string]::IsNullOrWhiteSpace($installPath)
}

$repoRoot = $PSScriptRoot
if ([string]::IsNullOrWhiteSpace($repoRoot)) {
    $repoRoot = (Get-Location).Path
}

Write-Host "[INFO] Checking Windows development prerequisites..." -ForegroundColor Cyan

if (-not (Test-MsvcBuildTools)) {
    Fail "MSVC Build Tools not detected. Install Visual Studio 2022 Build Tools with C++ workload."
}

Require-Command -Name "cmake" -InstallHint "Install CMake and ensure it is in PATH."
Require-Command -Name "node" -InstallHint "Install Node.js LTS."
Require-Command -Name "npm" -InstallHint "Install npm (bundled with Node.js)."

$cargoCmd = Get-Command "cargo" -ErrorAction SilentlyContinue
$rustupCmd = Get-Command "rustup" -ErrorAction SilentlyContinue
if (-not $cargoCmd -and -not $rustupCmd) {
    Fail "Rust toolchain not detected. Install Rust from https://rustup.rs/."
}

$uiProcess = Get-Process -Name "zonky-ui" -ErrorAction SilentlyContinue
if ($uiProcess) {
    if ($KillExisting) {
        Write-Host "[WARN] Existing zonky-ui process detected. Stopping it..." -ForegroundColor Yellow
        $uiProcess | Stop-Process -Force
    } else {
        Fail "zonky-ui.exe is already running. Close it or re-run with -KillExisting."
    }
}

Write-Host "[OK] All prerequisite checks passed." -ForegroundColor Green

if ($CheckOnly) {
    Write-Host "[INFO] Check-only mode enabled. Skipping UI launch." -ForegroundColor Cyan
    exit 0
}

$uiDir = Join-Path $repoRoot "crates\zonky-ui"
if (-not (Test-Path $uiDir)) {
    Fail "UI directory not found at $uiDir"
}

Push-Location $uiDir
try {
    Write-Host "[INFO] Launching Tauri UI from $uiDir" -ForegroundColor Cyan
    if ($cargoCmd) {
        & cargo tauri dev
    } else {
        & rustup run stable cargo tauri dev
    }
}
finally {
    Pop-Location
}
