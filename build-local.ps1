# Local Build Script for AIVORelay (Windows)
# Builds the project locally without code signing, similar to GitHub Actions

param(
    [switch]$SkipChecks,
    [switch]$Debug
)

$ErrorActionPreference = "Stop"

Write-Host ""
Write-Host "=======================================" -ForegroundColor Cyan
Write-Host " AIVORelay Local Build (Unsigned)" -ForegroundColor Cyan
Write-Host "=======================================" -ForegroundColor Cyan
Write-Host ""

# Function to check if command exists
function Test-Command($command) {
    $null -ne (Get-Command $command -ErrorAction SilentlyContinue)
}

# Step 1: Check for running processes
if (-not $SkipChecks) {
    Write-Host "[1/6] Checking for running cargo/tauri processes..." -ForegroundColor Yellow
    $runningProcs = Get-Process | Where-Object { $_.Name -match "cargo|tauri|rustc|bun" }

    if ($runningProcs) {
        Write-Host ""
        Write-Host "WARNING: Found running processes that may interfere with build:" -ForegroundColor Red
        $runningProcs | Select-Object Name, Id | Format-Table
        Write-Host "Please close dev server and try again, or use -SkipChecks to ignore." -ForegroundColor Yellow
        exit 1
    }
    Write-Host "  OK - No conflicting processes found" -ForegroundColor Green
} else {
    Write-Host "[1/6] Skipping process check (-SkipChecks)" -ForegroundColor Gray
}

# Step 2: Setup Visual Studio environment (Get-Dev equivalent from AGENTS.md)
Write-Host ""
Write-Host "[2/6] Setting up Visual Studio build environment..." -ForegroundColor Yellow

if (-not $SkipChecks) {
    # Use Launch-VsDevShell.ps1 (same as user's working Get-Dev function)
    Write-Host "  Running Launch-VsDevShell.ps1..." -ForegroundColor Gray

    try {
        # Use the same hardcoded path as user's Get-Dev function
        # (VS Community, not BuildTools which vswhere may find)
        $launchScript = "C:\Program Files\Microsoft Visual Studio\2022\Community\Common7\Tools\Launch-VsDevShell.ps1"

        if (-not (Test-Path $launchScript)) {
            Write-Host "ERROR: Launch-VsDevShell.ps1 not found at $launchScript" -ForegroundColor Red
            Write-Host "Please install Visual Studio 2022 Community with C++ build tools." -ForegroundColor Yellow
            exit 1
        }

        Write-Host "  Found VS Community at: C:\Program Files\Microsoft Visual Studio\2022\Community" -ForegroundColor Gray

        & $launchScript -Arch amd64 -HostArch amd64 -SkipAutomaticLocation

        Write-Host "  OK - Visual Studio environment configured" -ForegroundColor Green
    }
    catch {
        Write-Host "ERROR: Failed to setup VS environment: $_" -ForegroundColor Red
        exit 1
    }
} else {
    Write-Host "  Skipping VS environment setup (-SkipChecks)" -ForegroundColor Gray
}

# Step 3: Check Vulkan SDK
Write-Host ""
Write-Host "[3/6] Checking Vulkan SDK..." -ForegroundColor Yellow

if ($env:VULKAN_SDK) {
    Write-Host "  Found VULKAN_SDK: $env:VULKAN_SDK" -ForegroundColor Gray

    # Check for vulkan-1.dll
    $vulkanDll = Join-Path $env:VULKAN_SDK "Bin\vulkan-1.dll"
    $targetDir = "src-tauri"
    $targetDll = Join-Path $targetDir "vulkan-1.dll"

    if (Test-Path $vulkanDll) {
        # Copy vulkan-1.dll to src-tauri for bundling
        Write-Host "  Copying vulkan-1.dll to $targetDir for bundling..." -ForegroundColor Gray
        Copy-Item $vulkanDll -Destination $targetDir -Force
        $size = [math]::Round((Get-Item $targetDll).Length / 1MB, 2)
        Write-Host "  OK - Copied vulkan-1.dll $size MB" -ForegroundColor Green
    } else {
        Write-Host "  WARNING: vulkan-1.dll not found at $vulkanDll" -ForegroundColor Yellow
        Write-Host "  Trying System32 fallback..." -ForegroundColor Gray

        $systemDll = "C:\Windows\System32\vulkan-1.dll"
        if (Test-Path $systemDll) {
            Copy-Item $systemDll -Destination $targetDir -Force
            Write-Host "  OK - Copied vulkan-1.dll from System32" -ForegroundColor Green
        } else {
            Write-Host "  WARNING: Could not find vulkan-1.dll anywhere!" -ForegroundColor Yellow
            Write-Host "  Build may fail. Install Vulkan SDK from https://vulkan.lunarg.com/" -ForegroundColor Yellow
        }
    }
} else {
    Write-Host "  WARNING: VULKAN_SDK environment variable not set!" -ForegroundColor Yellow
    Write-Host "  Install from: https://vulkan.lunarg.com/sdk/home#windows" -ForegroundColor Yellow
    Write-Host "  Build will continue but may fail..." -ForegroundColor Yellow
}

# Step 4: Check required tools
Write-Host ""
Write-Host "[4/6] Checking required tools..." -ForegroundColor Yellow

$requiredTools = @{
    "bun" = "Bun package manager"
    "cargo" = "Rust toolchain"
}

$missingTools = @()
foreach ($tool in $requiredTools.Keys) {
    if (Test-Command $tool) {
        Write-Host "  OK - $($requiredTools[$tool])" -ForegroundColor Green
    } else {
        Write-Host "  ERROR - $($requiredTools[$tool]) NOT FOUND" -ForegroundColor Red
        $missingTools += $tool
    }
}

if ($missingTools.Count -gt 0) {
    Write-Host ""
    Write-Host "ERROR: Missing required tools: $($missingTools -join ', ')" -ForegroundColor Red
    Write-Host "Install from:" -ForegroundColor Yellow
    Write-Host "  - Bun: https://bun.sh/" -ForegroundColor Gray
    Write-Host "  - Rust: https://rustup.rs/" -ForegroundColor Gray
    exit 1
}

# Step 5: Install dependencies
Write-Host ""
Write-Host "[5/6] Installing dependencies..." -ForegroundColor Yellow
Write-Host "  Running: bun install" -ForegroundColor Gray

try {
    & bun install
    if ($LASTEXITCODE -ne 0) {
        throw "bun install failed with exit code $LASTEXITCODE"
    }
    Write-Host "  OK - Dependencies installed" -ForegroundColor Green
} catch {
    Write-Host "  ERROR: Failed to install dependencies: $_" -ForegroundColor Red
    exit 1
}

# Step 6: Build
Write-Host ""
Write-Host "[6/6] Building AIVORelay (unsigned)..." -ForegroundColor Yellow
Write-Host ""

if ($Debug) {
    Write-Host "  Building in DEBUG mode..." -ForegroundColor Cyan
    Write-Host "  Running: bun run tauri build --debug --no-sign" -ForegroundColor Gray
    Write-Host ""

    try {
        & bun run tauri build --debug --no-sign
        if ($LASTEXITCODE -ne 0) {
            throw "Build failed with exit code $LASTEXITCODE"
        }
    } catch {
        Write-Host ""
        Write-Host "ERROR: Debug build failed: $_" -ForegroundColor Red
        exit 1
    }
} else {
    Write-Host "  Building in RELEASE mode..." -ForegroundColor Cyan
    Write-Host "  Running: bun run build:unsigned" -ForegroundColor Gray
    Write-Host ""

    try {
        & bun run build:unsigned
        if ($LASTEXITCODE -ne 0) {
            throw "Build failed with exit code $LASTEXITCODE"
        }
    } catch {
        Write-Host ""
        Write-Host "ERROR: Release build failed: $_" -ForegroundColor Red
        exit 1
    }
}

# Success!
Write-Host ""
Write-Host "=======================================" -ForegroundColor Green
Write-Host " Build completed successfully!" -ForegroundColor Green
Write-Host "=======================================" -ForegroundColor Green
Write-Host ""

if ($Debug) {
    $bundlePath = "src-tauri\target\debug\bundle\msi"
} else {
    $bundlePath = "src-tauri\target\release\bundle\msi"
}

if (Test-Path $bundlePath) {
    Write-Host "Build artifacts:" -ForegroundColor Cyan
    Get-ChildItem $bundlePath -Filter *.msi | ForEach-Object {
        $sizeMB = [math]::Round($_.Length / 1MB, 2)
        $name = $_.Name
        Write-Host "  - $name - $sizeMB MB" -ForegroundColor Gray
    }
}

Write-Host ""
Write-Host "Note: This is an unsigned build without auto-update functionality" -ForegroundColor Yellow
Write-Host ""
