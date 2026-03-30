# Linker benchmark for AIVORelay (Windows, x64, unsigned Tauri builds)
# Compares the default MSVC linker path against suitable alternatives.

param(
    [string[]]$Linkers = @("default", "rust-lld", "lld-link"),
    [switch]$SkipChecks,
    [switch]$Debug,
    [switch]$WarmCache,
    [string]$OutputDir = ".AGENTS/.UNTRACKED",
    [string]$BaseTargetDir = ""
)

$ErrorActionPreference = "Stop"

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
        Write-Host "WARNING: No valid MSVC/Windows SDK include paths found for bindgen" -ForegroundColor Yellow
        return
    }

    $bindgenVar = "BINDGEN_EXTRA_CLANG_ARGS_x86_64_pc_windows_msvc"
    $bindgenArgs = "--target=x86_64-pc-windows-msvc $(($includePaths | ForEach-Object { "-isystem '$_'" }) -join ' ')"
    Set-Item -Path "Env:$bindgenVar" -Value $bindgenArgs
    Write-Host "Configured $bindgenVar for MSVC headers" -ForegroundColor Green

    if (-not $env:LIBCLANG_PATH) {
        $defaultLibclangPath = "C:\Program Files\LLVM\bin"
        if (Test-Path (Join-Path $defaultLibclangPath "libclang.dll")) {
            $env:LIBCLANG_PATH = $defaultLibclangPath
            Write-Host "Set LIBCLANG_PATH to $defaultLibclangPath" -ForegroundColor Green
        }
    }

    if (Test-Path "C:\Program Files\LLVM\bin\clang.exe") {
        $env:PATH = "C:\Program Files\LLVM\bin;$env:PATH"
        Write-Host "Added LLVM bin to PATH for bindgen" -ForegroundColor Green
    }
}

function Resolve-BaseTargetDir {
    param([string]$Preferred)

    if ($Preferred) {
        New-Item -ItemType Directory -Force -Path $Preferred | Out-Null
        return (Resolve-Path $Preferred).Path
    }

    $candidates = @("C:\b\l", "D:\t\l", "C:\t\l")
    foreach ($candidate in $candidates) {
        try {
            New-Item -ItemType Directory -Force -Path $candidate | Out-Null
            return (Resolve-Path $candidate).Path
        } catch {
            continue
        }
    }

    New-Item -ItemType Directory -Force -Path "C:\t\l" | Out-Null
    return (Resolve-Path "C:\t\l").Path
}

function Initialize-DevEnvironment {
    $vsWhere = Join-Path ${env:ProgramFiles(x86)} "Microsoft Visual Studio\Installer\vswhere.exe"
    if (-not (Test-Path $vsWhere)) {
        throw "vswhere.exe not found at $vsWhere"
    }

    $vsPath = & $vsWhere -latest -products * -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath
    if (-not $vsPath) {
        throw "Visual Studio installation with VC tools not found."
    }

    $vsDevCmd = Join-Path $vsPath "Common7\Tools\VsDevCmd.bat"
    if (-not (Test-Path $vsDevCmd)) {
        throw "VsDevCmd.bat not found at $vsDevCmd"
    }

    Write-Host "--- START TASK ---" -ForegroundColor DarkGray
    $vars = cmd /c "`"$vsDevCmd`" -arch=x64 -host_arch=x64 && set"
    Write-Host "--- END TASK ---" -ForegroundColor DarkGray

    if ($LASTEXITCODE -ne 0) {
        throw "VsDevCmd.bat failed with exit code $LASTEXITCODE"
    }

    foreach ($line in $vars) {
        if ($line -match '^(.+?)=(.*)$') {
            Set-Item -Path "Env:$($Matches[1])" -Value $Matches[2]
        }
    }

    $clPath = $null
    $linkPath = $null
    $vcBinDir = $null
    if ($env:VCToolsInstallDir) {
        $vcBinDir = Join-Path $env:VCToolsInstallDir "bin\Hostx64\x64"
        $candidateCl = Join-Path $vcBinDir "cl.exe"
        $candidateLink = Join-Path $vcBinDir "link.exe"
        if (Test-Path $candidateCl) {
            $clPath = $candidateCl
        }
        if (Test-Path $candidateLink) {
            $linkPath = $candidateLink
        }
    }

    if ($vcBinDir -and (Test-Path $vcBinDir) -and ($env:PATH -notlike "*$vcBinDir*")) {
        $env:PATH = "$vcBinDir;$env:PATH"
    }

    if ($clPath) {
        $env:CC = $clPath
        $env:CXX = $clPath
        $env:CMAKE_C_COMPILER = $clPath
        $env:CMAKE_CXX_COMPILER = $clPath
        Write-Host "Pinned C/C++ compiler to $clPath" -ForegroundColor Green
    }

    if ($linkPath) {
        $env:CMAKE_LINKER = $linkPath
    }

    Set-BindgenWindowsEnv
}

function Ensure-VulkanLoader {
    $targetDir = "src-tauri"
    $targetDll = Join-Path $targetDir "vulkan-1.dll"

    if (Test-Path $targetDll) {
        $size = [math]::Round((Get-Item $targetDll).Length / 1MB, 2)
        Write-Host "vulkan-1.dll already exists ($size MB)" -ForegroundColor Green
        return
    }

    $copied = $false
    if ($env:VULKAN_SDK) {
        $sdkDll = Join-Path $env:VULKAN_SDK "Bin\vulkan-1.dll"
        if (Test-Path $sdkDll) {
            Copy-Item $sdkDll -Destination $targetDir -Force
            $copied = $true
            Write-Host "Copied vulkan-1.dll from Vulkan SDK" -ForegroundColor Green
        }
    }

    if (-not $copied) {
        $systemDll = "C:\Windows\System32\vulkan-1.dll"
        if (Test-Path $systemDll) {
            Copy-Item $systemDll -Destination $targetDir -Force
            $copied = $true
            Write-Host "Copied vulkan-1.dll from System32" -ForegroundColor Green
        }
    }

    if (-not $copied) {
        throw "Could not find vulkan-1.dll in Vulkan SDK or System32."
    }
}

function Ensure-RequiredTools {
    $requiredTools = @("bun", "cargo", "rustc")
    $missing = @()
    foreach ($tool in $requiredTools) {
        if (-not (Test-Command $tool)) {
            $missing += $tool
        }
    }

    if ($missing.Count -gt 0) {
        throw "Missing required tools: $($missing -join ', ')"
    }
}

function Install-Dependencies {
    Write-Host "--- START TASK ---" -ForegroundColor DarkGray
    & bun install
    $exitCode = $LASTEXITCODE
    Write-Host "--- END TASK ---" -ForegroundColor DarkGray

    if ($exitCode -ne 0) {
        throw "bun install failed with exit code $exitCode"
    }
}

function Resolve-LinkerCandidates {
    $sysroot = (& rustc --print sysroot).Trim()
    $rustLldPath = Join-Path $sysroot "lib\rustlib\x86_64-pc-windows-msvc\bin\rust-lld.exe"
    $llvmLldPath = "C:\Program Files\LLVM\bin\lld-link.exe"

    return @(
        [pscustomobject]@{
            Name = "default"
            Label = "current-default"
            DirName = "d"
            LinkerPath = $null
            Notes = "Cargo default linker for x86_64-pc-windows-msvc (expected MSVC link.exe)."
        }
        [pscustomobject]@{
            Name = "rust-lld"
            Label = "rust-lld"
            DirName = "r"
            LinkerPath = (Test-Path $rustLldPath) ? $rustLldPath : $null
            Notes = "rust-lld from the active Rust toolchain."
        }
        [pscustomobject]@{
            Name = "lld-link"
            Label = "llvm-lld-link"
            DirName = "l"
            LinkerPath = (Test-Path $llvmLldPath) ? $llvmLldPath : $null
            Notes = "lld-link.exe from the standalone LLVM installation."
        }
    )
}

function Remove-BenchmarkDirectory {
    param(
        [string]$Path,
        [string]$BasePath
    )

    if (-not (Test-Path $Path)) {
        return
    }

    $resolvedPath = [System.IO.Path]::GetFullPath($Path)
    $resolvedBase = [System.IO.Path]::GetFullPath($BasePath)

    if (-not $resolvedPath.StartsWith($resolvedBase, [System.StringComparison]::OrdinalIgnoreCase)) {
        throw "Refusing to remove path outside benchmark root: $resolvedPath"
    }

    Remove-Item -LiteralPath $resolvedPath -Recurse -Force
}

function Invoke-BenchmarkBuild {
    param(
        [pscustomobject]$Candidate,
        [string]$RootOutputDir,
        [string]$TargetRoot,
        [string]$ProfileName
    )

    $result = [ordered]@{
        name = $Candidate.Name
        label = $Candidate.Label
        linkerPath = $Candidate.LinkerPath
        notes = $Candidate.Notes
        status = "skipped"
        elapsedSeconds = $null
        targetDir = $null
        logPath = $null
        warmupLogPath = $null
        artifactPath = $null
        failure = $null
    }

    if ($Candidate.Name -ne "default" -and -not $Candidate.LinkerPath) {
        $result.failure = "Linker binary was not found on this machine."
        return [pscustomobject]$result
    }

    $targetLeaf = if ($Candidate.PSObject.Properties.Name -contains "DirName" -and $Candidate.DirName) { $Candidate.DirName } else { $Candidate.Name }
    $targetDir = Join-Path $TargetRoot $targetLeaf
    $logPath = Join-Path $RootOutputDir "$($Candidate.Name).log"
    $warmupLogPath = Join-Path $RootOutputDir "$($Candidate.Name).warmup.log"
    $result.targetDir = $targetDir
    $result.logPath = $logPath
    $result.warmupLogPath = $warmupLogPath

    Remove-BenchmarkDirectory -Path $targetDir -BasePath $TargetRoot
    New-Item -ItemType Directory -Force -Path $targetDir | Out-Null

    $overrideConfig = '{"bundle":{"createUpdaterArtifacts":false}}'
    $tauriArgs = @("run", "tauri", "build", "--no-sign", "--config", $overrideConfig)
    if ($Debug) {
        $tauriArgs += "--debug"
    }

    $previousTargetDir = $env:CARGO_TARGET_DIR
    $previousLinker = $env:CARGO_TARGET_X86_64_PC_WINDOWS_MSVC_LINKER

    try {
        $env:CARGO_TARGET_DIR = $targetDir
        if ($Candidate.LinkerPath) {
            $env:CARGO_TARGET_X86_64_PC_WINDOWS_MSVC_LINKER = $Candidate.LinkerPath
        } else {
            Remove-Item Env:CARGO_TARGET_X86_64_PC_WINDOWS_MSVC_LINKER -ErrorAction SilentlyContinue
        }

        Write-Host ""
        Write-Host "Benchmarking $($Candidate.Label)" -ForegroundColor Cyan
        Write-Host "  Target dir: $targetDir" -ForegroundColor Gray
        if ($Candidate.LinkerPath) {
            Write-Host "  Linker: $($Candidate.LinkerPath)" -ForegroundColor Gray
        } else {
            Write-Host "  Linker: Cargo default" -ForegroundColor Gray
        }

        if ($WarmCache) {
            Write-Host "  Warm cache: priming build before timing" -ForegroundColor Gray
            Write-Host "--- START TASK ---" -ForegroundColor DarkGray
            & bun @tauriArgs 2>&1 | Tee-Object -FilePath $warmupLogPath | Out-Null
            $warmupExitCode = $LASTEXITCODE
            Write-Host "--- END TASK ---" -ForegroundColor DarkGray

            if ($warmupExitCode -ne 0) {
                $result.status = "failed"
                $result.failure = "Warm-cache priming build exited with code $warmupExitCode."
                return [pscustomobject]$result
            }
        }

        $stopwatch = [System.Diagnostics.Stopwatch]::StartNew()
        Write-Host "--- START TASK ---" -ForegroundColor DarkGray
        & bun @tauriArgs 2>&1 | Tee-Object -FilePath $logPath | Out-Null
        $exitCode = $LASTEXITCODE
        Write-Host "--- END TASK ---" -ForegroundColor DarkGray
        $stopwatch.Stop()

        $result.elapsedSeconds = [math]::Round($stopwatch.Elapsed.TotalSeconds, 2)

        if ($exitCode -ne 0) {
            $result.status = "failed"
            $result.failure = "Build exited with code $exitCode."
            return [pscustomobject]$result
        }

        $artifactRoot = if ($Debug) {
            Join-Path $targetDir "debug\bundle"
        } else {
            Join-Path $targetDir "release\bundle"
        }

        $result.status = "ok"
        $result.artifactPath = $artifactRoot
        return [pscustomobject]$result
    }
    catch {
        $result.status = "failed"
        $result.failure = $_.Exception.Message
        return [pscustomobject]$result
    }
    finally {
        $env:CARGO_TARGET_DIR = $previousTargetDir
        if ($previousLinker) {
            $env:CARGO_TARGET_X86_64_PC_WINDOWS_MSVC_LINKER = $previousLinker
        } else {
            Remove-Item Env:CARGO_TARGET_X86_64_PC_WINDOWS_MSVC_LINKER -ErrorAction SilentlyContinue
        }
    }
}

Write-Host ""
Write-Host "=======================================" -ForegroundColor Cyan
Write-Host " AIVORelay Linker Benchmark" -ForegroundColor Cyan
Write-Host "=======================================" -ForegroundColor Cyan
Write-Host ""

if (-not $SkipChecks) {
    Write-Host "[1/6] Checking for running cargo/tauri/rustc/bun processes..." -ForegroundColor Yellow
    $runningProcs = Get-Process | Where-Object { $_.Name -match "cargo|tauri|rustc|bun" }
    if ($runningProcs) {
        Write-Host ""
        Write-Host "Conflicting processes detected:" -ForegroundColor Red
        $runningProcs | Select-Object Name, Id | Format-Table
        throw "Close the running build/dev processes and retry."
    }
    Write-Host "No conflicting processes found" -ForegroundColor Green
} else {
    Write-Host "[1/6] Skipping process check (-SkipChecks)" -ForegroundColor Gray
}

Write-Host ""
Write-Host "[2/6] Preparing Visual Studio environment..." -ForegroundColor Yellow
Initialize-DevEnvironment

Write-Host ""
Write-Host "[3/6] Verifying Vulkan loader..." -ForegroundColor Yellow
Ensure-VulkanLoader

Write-Host ""
Write-Host "[4/6] Checking required tools..." -ForegroundColor Yellow
Ensure-RequiredTools

$timestamp = Get-Date -Format "yyyyMMdd-HHmmss"
$resolvedOutputDir = Join-Path $OutputDir "linker-benchmark-$timestamp"
New-Item -ItemType Directory -Force -Path $resolvedOutputDir | Out-Null
$resolvedOutputDir = (Resolve-Path $resolvedOutputDir).Path
$resolvedBaseTargetDir = Resolve-BaseTargetDir -Preferred $BaseTargetDir

Write-Host ""
Write-Host "[5/6] Installing frontend dependencies once..." -ForegroundColor Yellow
Install-Dependencies

Write-Host ""
Write-Host "[6/6] Running benchmark builds..." -ForegroundColor Yellow
Write-Host "Benchmark reports: $resolvedOutputDir" -ForegroundColor Gray
Write-Host "Benchmark target root: $resolvedBaseTargetDir" -ForegroundColor Gray

$allCandidates = Resolve-LinkerCandidates
$selectedCandidates = foreach ($name in $Linkers) {
    $match = $allCandidates | Where-Object { $_.Name -eq $name } | Select-Object -First 1
    if (-not $match) {
        throw "Unknown linker '$name'. Supported values: $((($allCandidates | Select-Object -ExpandProperty Name) -join ', '))"
    }
    $match
}

$profileName = if ($Debug) { "debug" } else { "release" }
$results = @()
foreach ($candidate in $selectedCandidates) {
    $results += Invoke-BenchmarkBuild -Candidate $candidate -RootOutputDir $resolvedOutputDir -TargetRoot $resolvedBaseTargetDir -ProfileName $profileName
}

$successful = $results | Where-Object { $_.status -eq "ok" } | Sort-Object elapsedSeconds
$baseline = $results | Where-Object { $_.name -eq "default" } | Select-Object -First 1

$summaryRows = foreach ($result in $results) {
    $speedup = ""
    if ($baseline -and $baseline.status -eq "ok" -and $result.status -eq "ok" -and $result.elapsedSeconds -gt 0) {
        $ratio = $baseline.elapsedSeconds / $result.elapsedSeconds
        $speedup = "{0:N2}x" -f $ratio
    }

    [pscustomobject]@{
        Linker = $result.label
        Status = $result.status
        Seconds = $result.elapsedSeconds
        SpeedupVsDefault = $speedup
        TargetDir = $result.targetDir
        Log = $result.logPath
        Failure = $result.failure
    }
}

$summaryJsonPath = Join-Path $resolvedOutputDir "summary.json"
$summaryMdPath = Join-Path $resolvedOutputDir "summary.md"

$results | ConvertTo-Json -Depth 5 | Set-Content -Path $summaryJsonPath

$mdLines = @()
$mdLines += "# AIVORelay linker benchmark"
$mdLines += ""
$mdLines += "- Timestamp: $(Get-Date -Format "yyyy-MM-dd HH:mm:ss zzz")"
$mdLines += "- Profile: $profileName"
$mdLines += "- Warm cache: $WarmCache"
$mdLines += "- Workspace: $((Get-Location).Path)"
$mdLines += "- Target root: $resolvedBaseTargetDir"
$mdLines += ""
$mdLines += "| Linker | Status | Seconds | Speedup vs default |"
$mdLines += "| --- | --- | ---: | ---: |"
foreach ($row in $summaryRows) {
    $seconds = if ($null -ne $row.Seconds) { $row.Seconds } else { "" }
    $mdLines += "| $($row.Linker) | $($row.Status) | $seconds | $($row.SpeedupVsDefault) |"
}
$mdLines += ""
foreach ($result in $results) {
    $mdLines += "## $($result.label)"
    $mdLines += ""
    $mdLines += "- Status: $($result.status)"
    $mdLines += "- Notes: $($result.notes)"
    if ($null -ne $result.elapsedSeconds) {
        $mdLines += "- Seconds: $($result.elapsedSeconds)"
    }
    if ($result.linkerPath) {
        $mdLines += "- Linker path: $($result.linkerPath)"
    }
    if ($result.targetDir) {
        $mdLines += "- Target dir: $($result.targetDir)"
    }
    if ($result.logPath) {
        $mdLines += "- Log: $($result.logPath)"
    }
    if ($WarmCache -and $result.warmupLogPath) {
        $mdLines += "- Warmup log: $($result.warmupLogPath)"
    }
    if ($result.failure) {
        $mdLines += "- Failure: $($result.failure)"
    }
    $mdLines += ""
}

$mdLines | Set-Content -Path $summaryMdPath

Write-Host ""
Write-Host "Summary" -ForegroundColor Cyan
$summaryRows | Format-Table -AutoSize

Write-Host ""
Write-Host "JSON summary: $summaryJsonPath" -ForegroundColor Gray
Write-Host "Markdown summary: $summaryMdPath" -ForegroundColor Gray

if ($successful.Count -gt 0) {
    Write-Host ""
    Write-Host "Fastest successful linker: $($successful[0].label) in $($successful[0].elapsedSeconds)s" -ForegroundColor Green
}
