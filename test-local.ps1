param(
    [switch]$SkipChecks,
    [switch]$LibOnly,
    [switch]$List,
    [switch]$Exact,
    [string]$Filter,
    [string]$CargoTargetDir
)

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

. "$PSScriptRoot\scripts\setup-rust-build-env.ps1"

Write-Host ""
Write-Host "=======================================" -ForegroundColor Cyan
Write-Host " AIVORelay Local Test Harness" -ForegroundColor Cyan
Write-Host "=======================================" -ForegroundColor Cyan
Write-Host ""

$context = $null

try {
    Write-Host "[1/3] Preparing Rust build environment..." -ForegroundColor Yellow
    $context = Initialize-RustBuildEnvironment `
        -SkipProcessCheck:$SkipChecks `
        -PreferredCargoTargetDir $CargoTargetDir `
        -MinimumFreeSpaceGB 50

    Write-Host "  OK - Using CARGO_TARGET_DIR $($context.CargoTargetDir)" -ForegroundColor Green
    Write-Host "  OK - Minimum free disk requirement enforced: $($context.MinimumFreeSpaceGB) GB" -ForegroundColor Green

    Write-Host ""
    Write-Host "[2/3] Building cargo test command..." -ForegroundColor Yellow
    $cargoArgs = @("test")
    if ($LibOnly) {
        $cargoArgs += "--lib"
    }
    if ($Filter) {
        $cargoArgs += $Filter
    }

    $testArgs = @()
    if ($List) {
        $testArgs += "--list"
    }
    if ($Exact) {
        $testArgs += "--exact"
    }
    if ($testArgs.Count -gt 0) {
        $cargoArgs += "--"
        $cargoArgs += $testArgs
    }

    Push-Location (Join-Path $PSScriptRoot "src-tauri")
    try {
        Write-Host ""
        Write-Host "[3/3] Running tests..." -ForegroundColor Yellow
        Write-Host "--- START TASK ---" -ForegroundColor DarkGray
        Write-Host ("  Running: cargo " + ($cargoArgs -join " ")) -ForegroundColor Gray
        & cargo @cargoArgs
        if ($LASTEXITCODE -ne 0) {
            throw "cargo test failed with exit code $LASTEXITCODE"
        }
        Write-Host "--- END TASK ---" -ForegroundColor DarkGray
    }
    finally {
        Pop-Location
    }

    Write-Host ""
    Write-Host "Tests completed successfully." -ForegroundColor Green
    Write-Host ""
}
finally {
    Restore-RustBuildEnvironment -Context $context
}
