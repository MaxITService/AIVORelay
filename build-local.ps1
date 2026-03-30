# Local Build Script for AIVORelay (Windows)
# Builds the project locally without code signing, similar to GitHub Actions

param(
    [switch]$SkipChecks,
    [switch]$Debug,
    [switch]$Avx2,
    [switch]$Cuda
)

if ($PSVersionTable.PSEdition -ne "Core" -or $PSVersionTable.PSVersion -lt [Version]"7.0") {
    Write-Error "This script requires pwsh (PowerShell 7+). Run: pwsh -NoProfile -File .\build-local.ps1"
    exit 1
}

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

function Get-PreferredCargoTargetDir([bool]$UseAvx2 = $false) {
    if ($env:AIVORELAY_CARGO_TARGET_DIR) {
        return $env:AIVORELAY_CARGO_TARGET_DIR
    }

    $candidates = if ($UseAvx2) {
        @("C:\a2", "D:\a2", "C:\t\aivorelay-avx2")
    } else {
        @("C:\b", "D:\t", "C:\t\aivorelay-local-build")
    }

    foreach ($candidate in $candidates) {
        try {
            New-Item -ItemType Directory -Force -Path $candidate | Out-Null
            return $candidate
        } catch {
            continue
        }
    }

    return if ($UseAvx2) { "C:\t\aivorelay-avx2" } else { "C:\t\aivorelay-local-build" }
}

function Set-Avx2BuildEnv {
    $cmakeInclude = Resolve-Path "src-tauri\cmake\force_ggml_avx2.cmake" -ErrorAction Stop
    $env:RUSTFLAGS = "-C target-feature=+avx2"
    $env:CMAKE_PROJECT_INCLUDE_BEFORE = $cmakeInclude.Path
    Write-Host "  OK - Enabled AVX2 Rust/CMake build overrides" -ForegroundColor Green
}

function Invoke-PrepareAvx2Sidecar([bool]$ReleaseBuild = $false) {
    $sidecarArgs = @("run", "scripts/prepare-avx2-sidecar.js")
    if ($ReleaseBuild) {
        $sidecarArgs += "--release"
    }

    Write-Host ""
    Write-Host "[6/7] Preparing AVX2 sidecar executable..." -ForegroundColor Yellow
    Write-Host "  Running: bun $($sidecarArgs -join ' ')" -ForegroundColor Gray

    & bun @sidecarArgs
    if ($LASTEXITCODE -ne 0) {
        throw "AVX2 sidecar preparation failed with exit code $LASTEXITCODE"
    }

    Write-Host "  OK - AVX2 sidecar prepared" -ForegroundColor Green
}

function Invoke-PrepareCudaSidecar([bool]$ReleaseBuild = $false) {
    $sidecarArgs = @("run", "scripts/prepare-cuda-sidecar.js")
    if ($ReleaseBuild) {
        $sidecarArgs += "--release"
    }

    Write-Host ""
    Write-Host "[6/7] Preparing CUDA sidecar executable..." -ForegroundColor Yellow
    Write-Host "  Running: bun $($sidecarArgs -join ' ')" -ForegroundColor Gray

    & bun @sidecarArgs
    if ($LASTEXITCODE -ne 0) {
        throw "CUDA sidecar preparation failed with exit code $LASTEXITCODE"
    }

    Write-Host "  OK - CUDA sidecar prepared" -ForegroundColor Green
}

function Get-TauriBuildOverrideJson([bool]$IncludeCuda = $false, [bool]$DisableUpdaterArtifacts = $false) {
    $externalBin = @("binaries/aivorelay-avx2")
    if ($IncludeCuda) {
        $externalBin += "binaries/aivorelay-cuda"
    }

    $override = @{
        bundle = @{
            externalBin = $externalBin
        }
    }

    if ($DisableUpdaterArtifacts) {
        $override.bundle.createUpdaterArtifacts = $false
    }

    return ($override | ConvertTo-Json -Compress)
}

function Test-IsGitHubActions {
    return $env:GITHUB_ACTIONS -eq "true"
}

function Test-KeepBuildCache {
    $keepBuildCache = if ($null -ne $env:AIVORELAY_KEEP_BUILD_CACHE) {
        $env:AIVORELAY_KEEP_BUILD_CACHE
    } else {
        ""
    }

    return @("1", "true", "yes") -contains $keepBuildCache.ToLowerInvariant()
}

function Should-CleanupLocalBuildCache {
    return (-not (Test-IsGitHubActions)) -and (-not (Test-KeepBuildCache))
}

function Get-ManagedSidecarTargetDir([string]$EnvVarName, [string[]]$Candidates, [string]$Fallback) {
    $existing = [Environment]::GetEnvironmentVariable($EnvVarName, "Process")
    if ($existing) {
        return $existing
    }

    foreach ($candidate in $Candidates) {
        try {
            New-Item -ItemType Directory -Force -Path $candidate | Out-Null
            Set-Item -Path "Env:$EnvVarName" -Value $candidate
            return $candidate
        } catch {
            continue
        }
    }

    Set-Item -Path "Env:$EnvVarName" -Value $Fallback
    return $Fallback
}

function Copy-BuildFileIfExists([string]$SourcePath, [string]$DestinationPath) {
    if (Test-Path $SourcePath) {
        Copy-Item -LiteralPath $SourcePath -Destination $DestinationPath -Force
    }
}

function Copy-LatestMatchingFile([string]$Pattern, [string]$DestinationPath) {
    $candidate = Get-ChildItem -Path $Pattern -ErrorAction SilentlyContinue |
        Sort-Object LastWriteTime -Descending |
        Select-Object -First 1

    if ($candidate) {
        Copy-Item -LiteralPath $candidate.FullName -Destination $DestinationPath -Force
    }
}

function Export-LocalBuildArtifacts([string]$CargoTargetDir, [bool]$ReleaseBuild, [bool]$IncludeCuda) {
    $profileName = if ($ReleaseBuild) { "release" } else { "debug" }
    $artifactsDir = Join-Path $PSScriptRoot ".AGENTS\.UNTRACKED\build-artifacts\$profileName"

    if (Test-Path $artifactsDir) {
        Remove-Item -LiteralPath $artifactsDir -Recurse -Force
    }

    New-Item -ItemType Directory -Force -Path $artifactsDir | Out-Null

    $mainExe = Join-Path $CargoTargetDir "$profileName\aivorelay.exe"
    Copy-BuildFileIfExists $mainExe (Join-Path $artifactsDir "aivorelay.exe")
    Copy-LatestMatchingFile (Join-Path $PSScriptRoot "src-tauri\binaries\aivorelay-avx2-*.exe") (Join-Path $artifactsDir "aivorelay-avx2.exe")

    if ($IncludeCuda) {
        Copy-LatestMatchingFile (Join-Path $PSScriptRoot "src-tauri\binaries\aivorelay-cuda-*.exe") (Join-Path $artifactsDir "aivorelay-cuda.exe")
    }

    $bundlePath = Join-Path $CargoTargetDir "$profileName\bundle\msi"
    if (Test-Path $bundlePath) {
        Get-ChildItem -Path $bundlePath -Filter *.msi -ErrorAction SilentlyContinue | ForEach-Object {
            Copy-Item -LiteralPath $_.FullName -Destination (Join-Path $artifactsDir $_.Name) -Force
        }
    }

    return $artifactsDir
}

function Remove-ManagedBuildDirectories([string[]]$PathsToRemove) {
    foreach ($path in ($PathsToRemove | Where-Object { $_ } | Select-Object -Unique)) {
        if (-not (Test-Path $path)) {
            continue
        }

        $resolved = (Resolve-Path -LiteralPath $path).Path
        $root = [System.IO.Path]::GetPathRoot($resolved)
        if ([string]::IsNullOrWhiteSpace($root) -or $resolved.TrimEnd('\') -eq $root.TrimEnd('\')) {
            throw "Refusing to delete unsafe path: $resolved"
        }

        Remove-Item -LiteralPath $resolved -Recurse -Force
    }
}

# Step 1: Check for running processes
if (-not $SkipChecks) {
    Write-Host "[1/7] Checking for running cargo/tauri processes..." -ForegroundColor Yellow
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
    Write-Host "[1/7] Skipping process check (-SkipChecks)" -ForegroundColor Gray
}

# Step 2: Setup Visual Studio environment (Get-Dev equivalent from AGENTS.md)
Write-Host ""
Write-Host "[2/7] Setting up Visual Studio build environment..." -ForegroundColor Yellow

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
Write-Host "[3/7] Checking Vulkan DLL..." -ForegroundColor Yellow

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
Write-Host "[4/7] Checking required tools..." -ForegroundColor Yellow

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

$cargoTargetDirWasExplicit = [bool]$env:AIVORELAY_CARGO_TARGET_DIR
$avx2TargetDirWasExplicit = [bool]$env:AIVORELAY_AVX2_TARGET_DIR
$cudaTargetDirWasExplicit = [bool]$env:AIVORELAY_CUDA_TARGET_DIR
$cargoTargetDir = Get-PreferredCargoTargetDir $Avx2
$artifactsDir = $null
try {
    New-Item -ItemType Directory -Force -Path $cargoTargetDir | Out-Null
    $env:CARGO_TARGET_DIR = $cargoTargetDir
    Write-Host ""
    Write-Host "  OK - Using short CARGO_TARGET_DIR $cargoTargetDir for build" -ForegroundColor Green

    if ($Avx2) {
        Set-Avx2BuildEnv
    }

# Step 5: Install dependencies
    Write-Host ""
    Write-Host "[5/7] Installing dependencies..." -ForegroundColor Yellow
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

    try {
        if (-not $avx2TargetDirWasExplicit) {
            $null = Get-ManagedSidecarTargetDir "AIVORELAY_AVX2_TARGET_DIR" @("C:\a2", "D:\a2", "C:\t\a2") "C:\t\a2"
        }
        Invoke-PrepareAvx2Sidecar -ReleaseBuild:(-not $Debug)
    } catch {
        Write-Host "  ERROR: Failed to prepare AVX2 sidecar: $_" -ForegroundColor Red
        exit 1
    }

    if ($Cuda) {
        try {
            if (-not $cudaTargetDirWasExplicit) {
                $null = Get-ManagedSidecarTargetDir "AIVORELAY_CUDA_TARGET_DIR" @("C:\cu", "D:\cu", "C:\t\cu") "C:\t\cu"
            }
            Invoke-PrepareCudaSidecar -ReleaseBuild:(-not $Debug)
        } catch {
            Write-Host "  ERROR: Failed to prepare CUDA sidecar: $_" -ForegroundColor Red
            exit 1
        }
    }

    # Step 7: Build
    Write-Host ""
    Write-Host "[7/7] Building AIVORelay (unsigned)..." -ForegroundColor Yellow
    Write-Host ""

    if ($Debug) {
        Write-Host "  Building in DEBUG mode..." -ForegroundColor Cyan
        $tauriArgs = @("run", "tauri", "build", "--debug", "--no-sign")
        if ($Cuda) {
            $tauriArgs += @("--config", (Get-TauriBuildOverrideJson -IncludeCuda $true))
        }
        Write-Host "  Running: bun $($tauriArgs -join ' ')" -ForegroundColor Gray
        Write-Host ""

        try {
            & bun @tauriArgs
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
            if ($Avx2) {
                $env:AIVORELAY_BUILD_AVX2 = "1"
            } else {
                $env:AIVORELAY_BUILD_AVX2 = $null
            }
            if ($Cuda) {
                $env:AIVORELAY_BUILD_CUDA = "1"
            } else {
                $env:AIVORELAY_BUILD_CUDA = $null
            }
            $env:AIVORELAY_CARGO_TARGET_DIR = $cargoTargetDir
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

    if (Should-CleanupLocalBuildCache) {
        $artifactsDir = Export-LocalBuildArtifacts -CargoTargetDir $cargoTargetDir -ReleaseBuild:(-not $Debug) -IncludeCuda:$Cuda

        $pathsToRemove = @()
        if (-not $cargoTargetDirWasExplicit) {
            $pathsToRemove += $cargoTargetDir
        }
        if (-not $avx2TargetDirWasExplicit) {
            $pathsToRemove += $env:AIVORELAY_AVX2_TARGET_DIR
        }
        if ($Cuda -and -not $cudaTargetDirWasExplicit) {
            $pathsToRemove += $env:AIVORELAY_CUDA_TARGET_DIR
        }

        Remove-ManagedBuildDirectories -PathsToRemove $pathsToRemove
    }

    # Success!
    Write-Host ""
    Write-Host "=======================================" -ForegroundColor Green
    Write-Host " Build completed successfully!" -ForegroundColor Green
    Write-Host "=======================================" -ForegroundColor Green
    Write-Host ""

    if ($artifactsDir -and (Test-Path $artifactsDir)) {
        Write-Host "Build artifacts:" -ForegroundColor Cyan
        Get-ChildItem $artifactsDir -File | Sort-Object Name | ForEach-Object {
            $sizeMB = [math]::Round($_.Length / 1MB, 2)
            $name = $_.Name
            Write-Host "  - $name - $sizeMB MB" -ForegroundColor Gray
        }
        Write-Host ""
        Write-Host "Local build cache cleaned; preserved artifacts in $artifactsDir" -ForegroundColor Green
    } elseif ($Debug) {
        $bundlePath = Join-Path $cargoTargetDir "debug\bundle\msi"
        if (Test-Path $bundlePath) {
            Write-Host "Build artifacts:" -ForegroundColor Cyan
            Get-ChildItem $bundlePath -Filter *.msi | ForEach-Object {
                $sizeMB = [math]::Round($_.Length / 1MB, 2)
                $name = $_.Name
                Write-Host "  - $name - $sizeMB MB" -ForegroundColor Gray
            }
        }
    } else {
        $bundlePath = Join-Path $cargoTargetDir "release\bundle\msi"
        if (Test-Path $bundlePath) {
            Write-Host "Build artifacts:" -ForegroundColor Cyan
            Get-ChildItem $bundlePath -Filter *.msi | ForEach-Object {
                $sizeMB = [math]::Round($_.Length / 1MB, 2)
                $name = $_.Name
                Write-Host "  - $name - $sizeMB MB" -ForegroundColor Gray
            }
        }
    }

    Write-Host ""
    Write-Host "Note: This is an unsigned build without auto-update functionality" -ForegroundColor Yellow
    Write-Host ""
}
finally {
    $env:CARGO_TARGET_DIR = $null
    $env:AIVORELAY_CARGO_TARGET_DIR = $null
    $env:AIVORELAY_AVX2_TARGET_DIR = $null
    $env:AIVORELAY_CUDA_TARGET_DIR = $null
    $env:AIVORELAY_BUILD_AVX2 = $null
    $env:AIVORELAY_BUILD_CUDA = $null
    $env:RUSTFLAGS = $null
    $env:CMAKE_PROJECT_INCLUDE_BEFORE = $null
}
