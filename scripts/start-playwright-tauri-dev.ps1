param(
    [ValidateRange(1, 65535)]
    [int]$PlaywrightPort = 9333,
    [string]$CargoTargetDir = "C:\t\aivorelay-dev",
    [switch]$SkipProcessCheck
)

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

. "$PSScriptRoot\setup-rust-build-env.ps1"

function Ensure-AivoRelayVulkanDll {
    param([Parameter(Mandatory = $true)][string]$TargetRoot)

    $targetDll = Join-Path $TargetRoot "src-tauri\vulkan-1.dll"
    if (Test-Path -LiteralPath $targetDll) {
        return
    }

    $copySource = $null
    if ($env:VULKAN_SDK) {
        $sdkDll = Join-Path $env:VULKAN_SDK "Bin\vulkan-1.dll"
        if (Test-Path -LiteralPath $sdkDll) {
            $copySource = $sdkDll
        }
    }

    if (-not $copySource) {
        $systemDll = "C:\Windows\System32\vulkan-1.dll"
        if (Test-Path -LiteralPath $systemDll) {
            $copySource = $systemDll
        }
    }

    if (-not $copySource) {
        throw "Could not find vulkan-1.dll in VULKAN_SDK or C:\Windows\System32."
    }

    Copy-Item -LiteralPath $copySource -Destination $targetDll -Force
}

$repoRoot = Split-Path -Parent $PSScriptRoot
if (-not (Test-Path -LiteralPath $repoRoot)) {
    throw "Repo root not found: $repoRoot"
}

if (-not (Get-Command bun -ErrorAction SilentlyContinue)) {
    throw "'bun' not found in PATH; ensure bun is installed and available."
}

$context = $null
$previousPlaywrightPort = $env:PLAYWRIGHT_TAURI_REMOTE_DEBUGGING_PORT
$hadPreviousPlaywrightPort = Test-Path Env:PLAYWRIGHT_TAURI_REMOTE_DEBUGGING_PORT

try {
    Write-Host "--- START TASK ---" -ForegroundColor DarkGray
    Write-Host "Preparing AivoRelay Playwright dev launch..." -ForegroundColor Cyan

    $context = Initialize-RustBuildEnvironment `
        -SkipProcessCheck:$SkipProcessCheck `
        -PreferredCargoTargetDir $CargoTargetDir `
        -MinimumFreeSpaceGB 10

    Ensure-AivoRelayVulkanDll -TargetRoot $repoRoot

    $env:PLAYWRIGHT_TAURI_REMOTE_DEBUGGING_PORT = $PlaywrightPort.ToString()

    Write-Host "Using CARGO_TARGET_DIR=$($context.CargoTargetDir)" -ForegroundColor DarkGray
    Write-Host "Playwright CDP enabled on port $PlaywrightPort." -ForegroundColor Cyan
    Write-Host "Starting 'bun x tauri dev' in $repoRoot" -ForegroundColor Green

    Push-Location -LiteralPath $repoRoot
    try {
        & bun x tauri dev
        if ($LASTEXITCODE -ne 0) {
            throw "bun x tauri dev failed with exit code $LASTEXITCODE"
        }
    }
    finally {
        Pop-Location
    }
}
finally {
    if ($hadPreviousPlaywrightPort) {
        $env:PLAYWRIGHT_TAURI_REMOTE_DEBUGGING_PORT = $previousPlaywrightPort
    }
    else {
        Remove-Item Env:PLAYWRIGHT_TAURI_REMOTE_DEBUGGING_PORT -ErrorAction SilentlyContinue
    }

    Restore-RustBuildEnvironment -Context $context
    Write-Host "--- END TASK ---" -ForegroundColor DarkGray
}
