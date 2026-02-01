<#
.SYNOPSIS
    Build AivoRelay with CUDA 12.4 support
.DESCRIPTION
    V5 "Sentinel Edition" - Fixed bindgen header discovery.

    Key fixes over V4:
    - Dynamic -resource-dir discovery (fixes stdbool.h not found)
    - INCLUDE env var for libclang Windows SDK discovery
    - Fail-fast detection for bindgen silent fallback
    - Bindings validation (detects Linux types in generated code)
    - C++ parsing mode (-x c++, -std=c++14)
.NOTES
    Run from PowerShell (not CMD)
    Requires: VS 2022, CUDA 12.4, LLVM/Clang installed
#>

param(
    [switch]$Clean,
    [switch]$Build,
    [switch]$Check,
    [switch]$Full,
    [switch]$Dev
)

$ErrorActionPreference = "Continue"

Write-Host "========================================" -ForegroundColor Cyan
Write-Host "  AivoRelay CUDA 12.4 Build Script V5.1" -ForegroundColor Cyan
Write-Host "  (Enhanced Bundle & Dev Edition)" -ForegroundColor Cyan
Write-Host "========================================" -ForegroundColor Cyan

# ============================================
# 1. Load Visual Studio 2022 Environment
# ============================================
Write-Host "`n[1/5] Loading Visual Studio environment..." -ForegroundColor Yellow

# Find VS installation that has C++ tools (not just BuildTools shell)
$vswhere = "${env:ProgramFiles(x86)}\Microsoft Visual Studio\Installer\vswhere.exe"

# First, try to find installation with VC++ tools component
$vsPath = & $vswhere -latest -products * -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath

# Fallback: try any VS installation
if (-not $vsPath) {
    Write-Host "  No VS with VC++ tools found, trying any installation..." -ForegroundColor Yellow
    $vsPath = & $vswhere -latest -products * -property installationPath
}

if (-not $vsPath) {
    Write-Host "ERROR: Visual Studio not found!" -ForegroundColor Red
    exit 1
}

cmd /c "`"$vsPath\Common7\Tools\VsDevCmd.bat`" -arch=x64 && set" |
Where-Object { $_ -match '^(.+?)=(.*)$' } |
ForEach-Object { Set-Item "Env:$($Matches[1])" $Matches[2] }

Write-Host "  VS Path: $vsPath" -ForegroundColor Green

# ============================================
# 2. Configure CUDA 12.4
# ============================================
Write-Host "`n[2/5] Configuring CUDA 12.4..." -ForegroundColor Yellow

$env:CUDA_PATH = "C:\Program Files\NVIDIA GPU Computing Toolkit\CUDA\v12.4"
if (-not (Test-Path $env:CUDA_PATH)) {
    Write-Host "ERROR: CUDA 12.4 not found at $env:CUDA_PATH" -ForegroundColor Red
    Write-Host "  CUDA 12.4 is required (13.x needs C++17 which whisper-rs-sys 0.11 can't pass)" -ForegroundColor Yellow
    exit 1
}

$env:CMAKE_CUDA_COMPILER = "$env:CUDA_PATH\bin\nvcc.exe"

# Add CUDA 12.4 and Ninja to PATH
$cudaBin = "$env:CUDA_PATH\bin"
$ninjaPath = "$env:LOCALAPPDATA\Microsoft\WinGet\Links"

if ($env:PATH -notmatch [regex]::Escape($cudaBin)) {
    $env:PATH = "$cudaBin;$env:PATH"
}
if ($env:PATH -notmatch [regex]::Escape($ninjaPath)) {
    $env:PATH = "$ninjaPath;$env:PATH"
}

# CMake settings for CUDA
$env:CMAKE_CUDA_STANDARD = "17"
$env:CMAKE_CUDA_STANDARD_REQUIRED = "ON"
$env:CMAKE_CXX_STANDARD = "17"
$env:CMAKE_CXX_STANDARD_REQUIRED = "ON"
$env:CMAKE_GENERATOR = "Ninja"
$env:CMAKE_CUDA_ARCHITECTURES = "75;80;86;89"
$env:CUDAARCHS = "75;80;86;89"

Write-Host "  CUDA_PATH: $env:CUDA_PATH" -ForegroundColor Green
Write-Host "  CMAKE_GENERATOR: Ninja" -ForegroundColor Green

# ============================================
# 3. Configure Bindgen (V5 - Resource Dir Fix)
# ============================================
Write-Host "`n[3/5] Configuring Bindgen (V5 method)..." -ForegroundColor Yellow

# --- LLVM/Clang paths ---
$LLVM = "C:\Program Files\LLVM"
$LLVM_BIN = Join-Path $LLVM "bin"
$clangExe = Join-Path $LLVM_BIN "clang.exe"

if (-not (Test-Path $clangExe)) {
    Write-Host "ERROR: clang.exe not found at $clangExe" -ForegroundColor Red
    exit 1
}

# KEY FIX: Get resource directory directly from clang
# This is where stdbool.h and other builtin headers live
$resourceDir = (& $clangExe --print-resource-dir).Trim()
$clangBuiltinInclude = Join-Path $resourceDir "include"

Write-Host "  Clang Resource Dir: $resourceDir" -ForegroundColor Green
Write-Host "  Builtin Include: $clangBuiltinInclude" -ForegroundColor Green

# --- Windows SDK paths from VsDevCmd environment ---
$vcToolsPath = $env:VCToolsInstallDir
$windowsSdkDir = $env:WindowsSdkDir
$windowsSdkVer = $env:WindowsSDKVersion.TrimEnd("\")

# FALLBACK: If VCToolsInstallDir not set (common with Build Tools), detect manually
if (-not $vcToolsPath) {
    Write-Host "  VCToolsInstallDir not in env, detecting via vswhere..." -ForegroundColor Yellow

    # Get VS installation path (already have it from step 1)
    $msvcPath = Join-Path $vsPath "VC\Tools\MSVC"

    if (Test-Path $msvcPath) {
        # Get the latest MSVC version
        $latestMsvc = Get-ChildItem $msvcPath -Directory | Sort-Object Name -Descending | Select-Object -First 1
        if ($latestMsvc) {
            $vcToolsPath = $latestMsvc.FullName + "\"
            Write-Host "  Detected VCToolsInstallDir: $vcToolsPath" -ForegroundColor Green
        }
    }
}

# FALLBACK: If WindowsSdkDir not set, use default location
if (-not $windowsSdkDir) {
    $windowsSdkDir = "C:\Program Files (x86)\Windows Kits\10\"
    Write-Host "  Using default WindowsSdkDir: $windowsSdkDir" -ForegroundColor Yellow
}

# FALLBACK: If SDK version not set, detect latest
if (-not $windowsSdkVer) {
    $sdkIncludePath = Join-Path $windowsSdkDir "Include"
    if (Test-Path $sdkIncludePath) {
        $latestSdk = Get-ChildItem $sdkIncludePath -Directory | Where-Object { $_.Name -match "^10\." } | Sort-Object Name -Descending | Select-Object -First 1
        if ($latestSdk) {
            $windowsSdkVer = $latestSdk.Name
            Write-Host "  Detected WindowsSDKVersion: $windowsSdkVer" -ForegroundColor Green
        }
    }
}

if (-not $vcToolsPath -or -not $windowsSdkDir -or -not $windowsSdkVer) {
    Write-Host "ERROR: Could not detect VS/SDK paths" -ForegroundColor Red
    Write-Host "  VCToolsInstallDir: $vcToolsPath" -ForegroundColor Red
    Write-Host "  WindowsSdkDir: $windowsSdkDir" -ForegroundColor Red
    Write-Host "  WindowsSDKVersion: $windowsSdkVer" -ForegroundColor Red
    exit 1
}

$vcInclude = Join-Path $vcToolsPath "include"
$ucrtInclude = Join-Path $windowsSdkDir "Include\$windowsSdkVer\ucrt"
$umInclude = Join-Path $windowsSdkDir "Include\$windowsSdkVer\um"
$sharedInclude = Join-Path $windowsSdkDir "Include\$windowsSdkVer\shared"

Write-Host "  VC Include: $vcInclude" -ForegroundColor Green
Write-Host "  UCRT Include: $ucrtInclude" -ForegroundColor Green

# KEY FIX: Set INCLUDE env var - libclang on Windows reads this directly
$includeValue = "$vcInclude;$ucrtInclude;$umInclude;$sharedInclude"
$env:INCLUDE = $includeValue
[Environment]::SetEnvironmentVariable("INCLUDE", $includeValue, "Process")
Write-Host "  INCLUDE env var: SET" -ForegroundColor Green

# --- Build clang args with proper quoting ---
# Using array then joining to avoid escaping nightmares
$clangArgs = @(
    "--target=x86_64-pc-windows-msvc",
    "-resource-dir `"$resourceDir`"",
    "-isystem `"$clangBuiltinInclude`"",
    "-fms-compatibility",
    "-fms-extensions",
    "-fms-compatibility-version=19",
    "-x c++",
    "-std=c++14"
)
$clangArgsString = $clangArgs -join " "

# Set bindgen environment variables using .NET API (handles dashes in names)
[Environment]::SetEnvironmentVariable("LIBCLANG_PATH", $LLVM_BIN, "Process")
[Environment]::SetEnvironmentVariable("BINDGEN_EXTRA_CLANG_ARGS", $clangArgsString, "Process")
[Environment]::SetEnvironmentVariable("BINDGEN_EXTRA_CLANG_ARGS_x86_64-pc-windows-msvc", $clangArgsString, "Process")

# Force live binding generation (don't use bundled Linux bindings)
Remove-Item Env:WHISPER_DONT_GENERATE_BINDINGS -ErrorAction SilentlyContinue

# Try to disable layout tests (size assertions)
[Environment]::SetEnvironmentVariable("BINDGEN_NO_LAYOUT_TESTS", "1", "Process")
$env:BINDGEN_NO_LAYOUT_TESTS = "1"
Write-Host "  BINDGEN_NO_LAYOUT_TESTS: 1" -ForegroundColor Green

Write-Host "  LIBCLANG_PATH: $LLVM_BIN" -ForegroundColor Green
Write-Host "  BINDGEN_EXTRA_CLANG_ARGS:" -ForegroundColor Cyan
Write-Host "    $clangArgsString" -ForegroundColor Gray

# ============================================
# 4. Build or Check
# ============================================
Write-Host "`n[4/5] Executing Build/Check..." -ForegroundColor Yellow

$logFile = "cargo_check_cuda.txt"

if ($Clean) {
    Write-Host "  Cleaning build artifacts..." -ForegroundColor Cyan
    cargo clean -p whisper-rs-sys --manifest-path src-tauri/Cargo.toml 2>$null
    $buildDir = "src-tauri/target/debug/build"
    if (Test-Path $buildDir) {
        Get-ChildItem $buildDir -Directory -Filter "whisper-rs-sys-*" | Remove-Item -Recurse -Force
    }
}

$cargoOutput = ""
if ($Full) {
    Write-Host "  RUNNING FULL TAURI BUNDLE (CUDA ENABLED, UNSIGNED)..." -ForegroundColor Green
    # --no-sign: Code signing only works in GitHub Actions (cloud)
    # This automatically runs BEFORE build commands (frontend build)
    bun run tauri build --features cuda --no-sign
    $cargoExitCode = $LASTEXITCODE
}
elseif ($Dev) {
    Write-Host "  LAUNCHING TAURI DEV (CUDA ENABLED)..." -ForegroundColor Green
    # Launch dev mode with CUDA features
    bun run tauri dev --features cuda
    $cargoExitCode = $LASTEXITCODE
}
elseif ($Build) {
    Write-Host "  Running: cargo build --release --features cuda" -ForegroundColor Cyan
    $cargoOutput = cargo build --manifest-path src-tauri/Cargo.toml --release --no-default-features --features cuda 2>&1
    $cargoExitCode = $LASTEXITCODE
}
else {
    Write-Host "  Running: cargo check --features cuda" -ForegroundColor Cyan
    $cargoOutput = cargo check --manifest-path src-tauri/Cargo.toml --no-default-features --features cuda 2>&1
    $cargoExitCode = $LASTEXITCODE
}

# Capture output to log for cargo check/build
if ($cargoOutput) {
    $cargoOutput | Out-File -FilePath $logFile -Encoding utf8
    $cargoOutput | Write-Host
}

# ============================================
# 5. Validation (Fail-Fast + Bindings Check)
# ============================================
Write-Host "`n[5/5] Validating build..." -ForegroundColor Yellow

$buildFailed = $false
$failReasons = @()

# Check 1: Cargo exit code
if ($cargoExitCode -ne 0) {
    $buildFailed = $true
    $failReasons += "Cargo exited with code $cargoExitCode"
}

# Check 2: FAIL-FAST - Bindgen silent fallback detection
$logContent = Get-Content $logFile -Raw -ErrorAction SilentlyContinue
if ($logContent -match "Unable to generate bindings") {
    $buildFailed = $true
    $failReasons += "Bindgen failed silently (fell back to bundled Linux bindings)"
    Write-Host "  [FAIL-FAST] Detected 'Unable to generate bindings' in output!" -ForegroundColor Red
}

# Check 3: stdbool.h not found
if ($logContent -match "stdbool\.h.*not found") {
    $buildFailed = $true
    $failReasons += "stdbool.h not found (resource-dir not working)"
    Write-Host "  [ERROR] stdbool.h not found - try replacing -isystem with -I" -ForegroundColor Red
}

# Check 4: Validate generated bindings (look for Linux types)
$bindingsPattern = "src-tauri/target/*/build/whisper-rs-sys-*/out/bindings.rs"
$bindingsFiles = Get-ChildItem -Path $bindingsPattern -ErrorAction SilentlyContinue

if ($bindingsFiles) {
    foreach ($bindingsFile in $bindingsFiles) {
        $bindingsContent = Get-Content $bindingsFile.FullName -Raw -ErrorAction SilentlyContinue

        # Check for Linux-specific types that indicate wrong bindings
        if ($bindingsContent -match "_G_fpos_t|_IO_FILE|__off_t|__off64_t") {
            $buildFailed = $true
            $failReasons += "Generated bindings contain Linux types (_G_fpos_t) - bindgen used wrong platform"
            Write-Host "  [ERROR] Bindings contain Linux types: $($bindingsFile.FullName)" -ForegroundColor Red
        }
        else {
            Write-Host "  [OK] Bindings file looks Windows-native: $($bindingsFile.Name)" -ForegroundColor Green
        }

        # Check for the specific size assertion that causes overflow
        if ($bindingsContent -match "216usize") {
            $buildFailed = $true
            $failReasons += "Bindings contain 216-byte size check (Linux _G_fpos_t size)"
            Write-Host "  [ERROR] Bindings have Linux size assertions (216 bytes)" -ForegroundColor Red
        }
    }
}
else {
    Write-Host "  [INFO] No bindings.rs found yet (may be in cache)" -ForegroundColor Yellow
}

# ============================================
# Final Result
# ============================================
Write-Host "`n========================================" -ForegroundColor Cyan

if ($buildFailed) {
    Write-Host "  BUILD FAILED" -ForegroundColor Red
    Write-Host "========================================" -ForegroundColor Cyan
    Write-Host "`nReasons:" -ForegroundColor Yellow
    foreach ($reason in $failReasons) {
        Write-Host "  - $reason" -ForegroundColor Red
    }
    Write-Host "`nLog saved to: $logFile" -ForegroundColor Yellow

    # Diagnostic hints
    Write-Host "`nDiagnostic hints:" -ForegroundColor Yellow
    if ($failReasons -match "stdbool") {
        Write-Host "  1. Try replacing -isystem with -I in clangArgs" -ForegroundColor Cyan
        Write-Host "  2. Verify LLVM installation: $clangExe --version" -ForegroundColor Cyan
    }
    if ($failReasons -match "Linux") {
        Write-Host "  1. Run with -Clean to force rebinding" -ForegroundColor Cyan
        Write-Host "  2. Check BINDGEN_EXTRA_CLANG_ARGS is being read" -ForegroundColor Cyan
    }

    exit 1
}
else {
    Write-Host "  BUILD SUCCESSFUL!" -ForegroundColor Green
    Write-Host "========================================" -ForegroundColor Cyan
    exit 0
}
