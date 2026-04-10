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

function Get-PreferredCargoTargetDir {
    if ($env:AIVORELAY_CARGO_TARGET_DIR) {
        return $env:AIVORELAY_CARGO_TARGET_DIR
    }

    $candidates = @("Q:\b", "Q:\t\aivorelay-local-build", "D:\t", "C:\t\aivorelay-local-build")
    foreach ($candidate in $candidates) {
        try {
            New-Item -ItemType Directory -Force -Path $candidate | Out-Null
            return $candidate
        } catch {
            continue
        }
    }

    return "Q:\t\aivorelay-local-build"
}

function Get-NewestChildDirectory([string]$Path) {
    if (-not (Test-Path $Path)) {
        return $null
    }

    return Get-ChildItem -Path $Path -Directory -ErrorAction SilentlyContinue |
        Sort-Object Name -Descending |
        Select-Object -First 1
}

function Set-BindgenWindowsEnv {
    $includePaths = @()
    $msvcRoot = $null

    if ($env:VCToolsInstallDir -and (Test-Path (Join-Path $env:VCToolsInstallDir "include\vcruntime.h"))) {
        $includePaths += (Join-Path $env:VCToolsInstallDir "include")
    } else {
        $vsInstallPath = $env:VSINSTALLDIR
        if ($vsInstallPath) {
            $candidateRoot = Join-Path $vsInstallPath "VC\Tools\MSVC"
            if (Test-Path $candidateRoot) {
                $msvcRoot = $candidateRoot
            }
        }

        if (-not $msvcRoot) {
            $vsWhere = Join-Path ${env:ProgramFiles(x86)} "Microsoft Visual Studio\Installer\vswhere.exe"
            if (Test-Path $vsWhere) {
                $vsInstallPath = & $vsWhere -latest -products * -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath
                if ($vsInstallPath) {
                    $candidateRoot = Join-Path $vsInstallPath "VC\Tools\MSVC"
                    if (Test-Path $candidateRoot) {
                        $msvcRoot = $candidateRoot
                    }
                }
            }
        }
    }

    if ($msvcRoot) {
        $latestMsvcDir = Get-NewestChildDirectory $msvcRoot
        if ($latestMsvcDir) {
            $msvcInclude = Join-Path $latestMsvcDir.FullName "include"
            if (Test-Path $msvcInclude) {
                $includePaths += $msvcInclude
            }
        }
    }

    $windowsSdkRoot = Join-Path ${env:ProgramFiles(x86)} "Windows Kits\10\Include"
    $latestSdkDir = Get-NewestChildDirectory $windowsSdkRoot
    if ($latestSdkDir) {
        foreach ($subdir in @("ucrt", "shared", "um", "winrt", "cppwinrt")) {
            $candidate = Join-Path $latestSdkDir.FullName $subdir
            if (Test-Path $candidate) {
                $includePaths += $candidate
            }
        }
    }

    $includePaths = $includePaths | Select-Object -Unique
    if ($includePaths.Count -eq 0) {
        Write-Host "  WARNING: No valid MSVC/Windows SDK include paths found for bindgen" -ForegroundColor Yellow
        return
    }

    $bindgenVar = "BINDGEN_EXTRA_CLANG_ARGS_x86_64_pc_windows_msvc"
    $bindgenArgs = "--target=x86_64-pc-windows-msvc $(($includePaths | ForEach-Object { "-isystem '$_'" }) -join ' ')"
    Set-Item -Path "Env:$bindgenVar" -Value $bindgenArgs
    Write-Host "  OK - Configured $bindgenVar for MSVC headers" -ForegroundColor Green

    if (-not $env:LIBCLANG_PATH) {
        $defaultLibclangPath = "C:\Program Files\LLVM\bin"
        if (Test-Path (Join-Path $defaultLibclangPath "libclang.dll")) {
            $env:LIBCLANG_PATH = $defaultLibclangPath
            Write-Host "  OK - Set LIBCLANG_PATH to $defaultLibclangPath" -ForegroundColor Green
        }
    }

    if (Test-Path "C:\Program Files\LLVM\bin\clang.exe") {
        $env:PATH = "C:\Program Files\LLVM\bin;$env:PATH"
        Write-Host "  OK - Added LLVM bin to PATH for bindgen" -ForegroundColor Green
    }
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
    Write-Host "  Importing VS dev environment via vswhere + VsDevCmd.bat..." -ForegroundColor Gray

    try {
        $vsWhere = Join-Path ${env:ProgramFiles(x86)} "Microsoft Visual Studio\Installer\vswhere.exe"
        if (-not (Test-Path $vsWhere)) {
            Write-Host "ERROR: vswhere.exe not found at $vsWhere" -ForegroundColor Red
            exit 1
        }

        $vsPath = & $vsWhere -latest -products * -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath
        if (-not $vsPath) {
            Write-Host "ERROR: Visual Studio installation not found." -ForegroundColor Red
            exit 1
        }

        $vsDevCmd = Join-Path $vsPath "Common7\Tools\VsDevCmd.bat"
        if (-not (Test-Path $vsDevCmd)) {
            Write-Host "ERROR: VsDevCmd.bat not found at $vsDevCmd" -ForegroundColor Red
            exit 1
        }

        $vars = cmd /c "`"$vsDevCmd`" -arch=x64 -host_arch=x64 && set"
        if ($LASTEXITCODE -ne 0) {
            Write-Host "ERROR: VsDevCmd.bat failed with exit code $LASTEXITCODE" -ForegroundColor Red
            exit 1
        }

        foreach ($line in $vars) {
            if ($line -match '^(.+?)=(.*)$') {
                Set-Item -Path "Env:$($Matches[1])" -Value $Matches[2]
            }
        }

        Write-Host "  OK - Visual Studio environment configured" -ForegroundColor Green
        Set-BindgenWindowsEnv
    }
    catch {
        Write-Host "ERROR: Failed to setup VS environment: $_" -ForegroundColor Red
        exit 1
    }
} else {
    Write-Host "  Skipping VS environment setup (-SkipChecks)" -ForegroundColor Gray
}

# Step 3: Check Vulkan DLL
Write-Host ""
Write-Host "[3/6] Checking Vulkan DLL..." -ForegroundColor Yellow

$targetDir = "src-tauri"
$targetDll = Join-Path $targetDir "vulkan-1.dll"

if (Test-Path $targetDll) {
    # DLL already exists - skip copying
    $size = [math]::Round((Get-Item $targetDll).Length / 1MB, 2)
    Write-Host "  OK - vulkan-1.dll already exists ($size MB)" -ForegroundColor Green
} else {
    # Need to copy DLL
    Write-Host "  vulkan-1.dll not found in $targetDir, copying..." -ForegroundColor Gray

    $copied = $false

    # Try Vulkan SDK first (some versions have it in Bin)
    if ($env:VULKAN_SDK) {
        $sdkDll = Join-Path $env:VULKAN_SDK "Bin\vulkan-1.dll"
        if (Test-Path $sdkDll) {
            Copy-Item $sdkDll -Destination $targetDir -Force
            $copied = $true
            Write-Host "  OK - Copied from Vulkan SDK" -ForegroundColor Green
        }
    }

    # Fallback to System32 (installed with GPU drivers)
    if (-not $copied) {
        $systemDll = "C:\Windows\System32\vulkan-1.dll"
        if (Test-Path $systemDll) {
            Copy-Item $systemDll -Destination $targetDir -Force
            $copied = $true
            Write-Host "  OK - Copied from System32" -ForegroundColor Green
        }
    }

    if (-not $copied) {
        Write-Host "  WARNING: Could not find vulkan-1.dll anywhere!" -ForegroundColor Yellow
        Write-Host "  Build may fail. Install GPU drivers or Vulkan SDK." -ForegroundColor Yellow
    } else {
        $size = [math]::Round((Get-Item $targetDll).Length / 1MB, 2)
        Write-Host "  Size: $size MB" -ForegroundColor Gray
    }
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

$cargoTargetDir = Get-PreferredCargoTargetDir
try {
    New-Item -ItemType Directory -Force -Path $cargoTargetDir | Out-Null
    $env:CARGO_TARGET_DIR = $cargoTargetDir
    $env:AIVORELAY_CARGO_TARGET_DIR = $cargoTargetDir
    Write-Host ""
    Write-Host "  OK - Using short CARGO_TARGET_DIR $cargoTargetDir for build" -ForegroundColor Green

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
}
finally {
    $env:CARGO_TARGET_DIR = $null
    $env:AIVORELAY_CARGO_TARGET_DIR = $null
}
