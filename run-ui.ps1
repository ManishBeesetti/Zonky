param(
    [switch]$CheckOnly,
    [switch]$KillExisting
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

function Fail {
    param([string]$Message)
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
    return $cmd
}

function Test-MsvcBuildTools {
    if (-not ${env:ProgramFiles(x86)}) {
        return $false
    }

    $vswhere = Join-Path ${env:ProgramFiles(x86)} "Microsoft Visual Studio\Installer\vswhere.exe"
    if (-not (Test-Path $vswhere)) {
        return $false
    }

    $installPath = & $vswhere -latest -products * -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath
    return -not [string]::IsNullOrWhiteSpace($installPath)
}

function Test-ViteAlive {
    try {
        $resp = Invoke-WebRequest -Uri "http://127.0.0.1:5173" -UseBasicParsing -TimeoutSec 2
        return ($resp.StatusCode -ge 200 -and $resp.StatusCode -lt 500)
    } catch {
        return $false
    }
}

$repoRoot = $PSScriptRoot
if ([string]::IsNullOrWhiteSpace($repoRoot)) {
    $repoRoot = (Get-Location).Path
}

$uiRoot = Join-Path $repoRoot "crates\zonky-ui"
$frontendRoot = Join-Path $uiRoot "frontend"
$viteOutLog = Join-Path $frontendRoot "vite.out.log"
$viteErrLog = Join-Path $frontendRoot "vite.err.log"

if (-not (Test-Path $uiRoot)) {
    Fail "UI project not found: $uiRoot"
}

Write-Host "[INFO] Checking Windows development prerequisites..." -ForegroundColor Cyan

if (-not (Test-MsvcBuildTools)) {
    Fail "MSVC Build Tools not detected. Install Visual Studio 2022 Build Tools with C++ workload."
}

Require-Command -Name "cmake" -InstallHint "Install CMake and ensure it is in PATH." | Out-Null
Require-Command -Name "node" -InstallHint "Install Node.js LTS." | Out-Null
$npmCommand = Require-Command -Name "npm" -InstallHint "Install npm (bundled with Node.js)."
$npmExe = $npmCommand.Source

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

$nodeModules = Join-Path $frontendRoot "node_modules"
if (-not (Test-Path $nodeModules)) {
    Write-Host "[INFO] Installing frontend dependencies..." -ForegroundColor Cyan
    & $npmExe install --prefix $frontendRoot | Out-Host
}

if (-not (Test-ViteAlive)) {
    Write-Host "[INFO] Starting frontend dev server on http://127.0.0.1:5173 ..." -ForegroundColor Cyan
    if (Test-Path $viteOutLog) { Remove-Item $viteOutLog -Force }
    if (Test-Path $viteErrLog) { Remove-Item $viteErrLog -Force }

    Start-Process `
        -FilePath $npmExe `
        -ArgumentList @("run", "dev", "--", "--host", "127.0.0.1", "--port", "5173") `
        -WorkingDirectory $frontendRoot `
        -RedirectStandardOutput $viteOutLog `
        -RedirectStandardError $viteErrLog | Out-Null

    $ready = $false
    for ($i = 0; $i -lt 25; $i++) {
        Start-Sleep -Milliseconds 400
        if (Test-ViteAlive) {
            $ready = $true
            break
        }
    }

    if (-not $ready) {
        Fail "Vite failed to start. Check logs: $viteOutLog and $viteErrLog"
    }
}

Write-Host "[INFO] Launching zonky-ui..." -ForegroundColor Cyan
if ($cargoCmd) {
    & cargo run -p zonky-ui
} else {
    & rustup run stable cargo run -p zonky-ui
}
