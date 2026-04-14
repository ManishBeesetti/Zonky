Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

$repoRoot = Split-Path -Parent $MyInvocation.MyCommand.Path
$uiRoot = Join-Path $repoRoot "crates\zonky-ui"
$frontendRoot = Join-Path $uiRoot "frontend"
$viteOutLog = Join-Path $frontendRoot "vite.out.log"
$viteErrLog = Join-Path $frontendRoot "vite.err.log"

$vsDevCmd = "C:\Program Files (x86)\Microsoft Visual Studio\2022\BuildTools\Common7\Tools\VsDevCmd.bat"
$cargoExe = Join-Path $env:USERPROFILE ".cargo\bin\cargo.exe"
$npmCmd = "C:\Program Files\nodejs\npm.cmd"

if (-not (Test-Path $uiRoot)) {
    throw "UI project not found: $uiRoot"
}
if (-not (Test-Path $vsDevCmd)) {
    throw "VsDevCmd.bat not found: $vsDevCmd"
}
if (-not (Test-Path $cargoExe)) {
    throw "cargo.exe not found: $cargoExe"
}
if (-not (Test-Path $npmCmd)) {
    throw "npm.cmd not found: $npmCmd"
}

function Test-ViteAlive {
    try {
        $resp = Invoke-WebRequest -Uri "http://127.0.0.1:5173" -UseBasicParsing -TimeoutSec 2
        return ($resp.StatusCode -ge 200 -and $resp.StatusCode -lt 500)
    } catch {
        return $false
    }
}

# Install frontend dependencies once
$nodeModules = Join-Path $frontendRoot "node_modules"
if (-not (Test-Path $nodeModules)) {
    Write-Host "Installing frontend dependencies..."
    & $npmCmd install | Out-Host
}

# Start Vite dev server if needed
if (-not (Test-ViteAlive)) {
    Write-Host "Starting frontend dev server on http://127.0.0.1:5173 ..."
    if (Test-Path $viteOutLog) { Remove-Item $viteOutLog -Force }
    if (Test-Path $viteErrLog) { Remove-Item $viteErrLog -Force }

    Start-Process `
        -FilePath $npmCmd `
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
        throw "Vite failed to start. Check logs: $viteOutLog and $viteErrLog"
    }
}

Write-Host "Launching zonky-ui..."
$cmd = "call `"$vsDevCmd`" -arch=x64 >nul && `"$cargoExe`" run -p zonky-ui"
& cmd.exe /c $cmd
