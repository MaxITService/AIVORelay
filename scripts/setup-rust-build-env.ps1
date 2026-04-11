Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

function Test-Command {
    param([Parameter(Mandatory = $true)][string]$Command)

    $null -ne (Get-Command $Command -ErrorAction SilentlyContinue)
}

function Get-NewestChildDirectory {
    param([Parameter(Mandatory = $true)][string]$Path)

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
        $latestMsvcDir = Get-NewestChildDirectory -Path $msvcRoot
        if ($latestMsvcDir) {
            $msvcInclude = Join-Path $latestMsvcDir.FullName "include"
            if (Test-Path $msvcInclude) {
                $includePaths += $msvcInclude
            }
        }
    }

    $windowsSdkRoot = Join-Path ${env:ProgramFiles(x86)} "Windows Kits\10\Include"
    $latestSdkDir = Get-NewestChildDirectory -Path $windowsSdkRoot
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
        throw "No valid MSVC/Windows SDK include paths found for bindgen."
    }

    $bindgenVar = "BINDGEN_EXTRA_CLANG_ARGS_x86_64_pc_windows_msvc"
    $bindgenArgs = "--target=x86_64-pc-windows-msvc $(($includePaths | ForEach-Object { "-isystem '$_'" }) -join ' ')"
    Set-Item -Path "Env:$bindgenVar" -Value $bindgenArgs

    if (-not $env:LIBCLANG_PATH) {
        $defaultLibclangPath = "C:\Program Files\LLVM\bin"
        if (Test-Path (Join-Path $defaultLibclangPath "libclang.dll")) {
            $env:LIBCLANG_PATH = $defaultLibclangPath
        }
    }

    $llvmBinPath = "C:\Program Files\LLVM\bin"
    if (Test-Path (Join-Path $llvmBinPath "clang.exe")) {
        if (-not ($env:PATH -split ";" | Where-Object { $_ -eq $llvmBinPath })) {
            $env:PATH = "$llvmBinPath;$env:PATH"
        }
    }
}

function Get-PreferredCargoTargetDir {
    param([string]$PreferredPath)

    if ($PreferredPath) {
        return $PreferredPath
    }

    if ($env:AIVORELAY_CARGO_TARGET_DIR) {
        return $env:AIVORELAY_CARGO_TARGET_DIR
    }

    foreach ($candidate in @("Q:\t\c", "Q:\b", "D:\t\c", "C:\t\c")) {
        try {
            New-Item -ItemType Directory -Force -Path $candidate | Out-Null
            return $candidate
        } catch {
            continue
        }
    }

    return "Q:\t\c"
}

function Assert-MinimumFreeSpace {
    param(
        [Parameter(Mandatory = $true)][string]$Path,
        [Parameter(Mandatory = $true)][int]$MinimumFreeSpaceGB
    )

    $root = [System.IO.Path]::GetPathRoot($Path)
    if (-not $root) {
        throw "Unable to determine drive root for '$Path'."
    }

    $driveName = $root.TrimEnd("\").TrimEnd(":")
    $drive = Get-PSDrive -Name $driveName -ErrorAction Stop
    $minimumBytes = [int64]$MinimumFreeSpaceGB * 1GB

    if ($drive.Free -lt $minimumBytes) {
        $freeGB = [math]::Round($drive.Free / 1GB, 2)
        throw "At least $MinimumFreeSpaceGB GB of free disk space is required for tests. Drive $driveName has only $freeGB GB free."
    }
}

function Initialize-RustBuildEnvironment {
    param(
        [switch]$SkipProcessCheck,
        [string]$PreferredCargoTargetDir,
        [int]$MinimumFreeSpaceGB = 50
    )

    if (-not (Test-Command -Command "cargo")) {
        throw "cargo was not found in PATH."
    }

    if (-not $SkipProcessCheck) {
        $currentProcessId = $PID
        $currentProcess = Get-CimInstance Win32_Process -Filter "ProcessId = $currentProcessId" -ErrorAction SilentlyContinue
        $parentProcessId = if ($currentProcess) { [int]$currentProcess.ParentProcessId } else { -1 }

        $runningProcs = Get-Process -ErrorAction SilentlyContinue |
            Where-Object {
                $_.Name -match "cargo|tauri|rustc|bun" -and
                $_.Id -ne $currentProcessId -and
                $_.Id -ne $parentProcessId
            }
        if ($runningProcs) {
            $details = ($runningProcs | Select-Object Name, Id | Format-Table -HideTableHeaders | Out-String).Trim()
            throw "Found running cargo/tauri/rustc/bun processes. Stop them before running tests.`n$details"
        }
    }

    $vsWhere = Join-Path ${env:ProgramFiles(x86)} "Microsoft Visual Studio\Installer\vswhere.exe"
    if (-not (Test-Path $vsWhere)) {
        throw "vswhere.exe not found at $vsWhere"
    }

    $vsPath = & $vsWhere -latest -products * -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath
    if (-not $vsPath) {
        throw "Visual Studio with VC Tools was not found."
    }

    $vsDevCmd = Join-Path $vsPath "Common7\Tools\VsDevCmd.bat"
    if (-not (Test-Path $vsDevCmd)) {
        throw "VsDevCmd.bat not found at $vsDevCmd"
    }

    $vars = cmd /c "`"$vsDevCmd`" -arch=x64 -host_arch=x64 && set"
    if ($LASTEXITCODE -ne 0) {
        throw "VsDevCmd.bat failed with exit code $LASTEXITCODE"
    }

    foreach ($line in $vars) {
        if ($line -match '^(.+?)=(.*)$') {
            Set-Item -Path "Env:$($Matches[1])" -Value $Matches[2]
        }
    }

    Set-BindgenWindowsEnv

    $previousCargoTargetDir = $env:CARGO_TARGET_DIR
    $cargoTargetDir = Get-PreferredCargoTargetDir -PreferredPath $PreferredCargoTargetDir
    New-Item -ItemType Directory -Force -Path $cargoTargetDir | Out-Null
    Assert-MinimumFreeSpace -Path $cargoTargetDir -MinimumFreeSpaceGB $MinimumFreeSpaceGB
    $env:CARGO_TARGET_DIR = $cargoTargetDir

    return [pscustomobject]@{
        CargoTargetDir = $cargoTargetDir
        PreviousCargoTargetDir = $previousCargoTargetDir
        BindgenEnvVar = "BINDGEN_EXTRA_CLANG_ARGS_x86_64_pc_windows_msvc"
        MinimumFreeSpaceGB = $MinimumFreeSpaceGB
    }
}

function Restore-RustBuildEnvironment {
    param($Context)

    if ($null -eq $Context) {
        return
    }

    if ([string]::IsNullOrWhiteSpace($Context.PreviousCargoTargetDir)) {
        Remove-Item Env:CARGO_TARGET_DIR -ErrorAction SilentlyContinue
    } else {
        $env:CARGO_TARGET_DIR = $Context.PreviousCargoTargetDir
    }
}
