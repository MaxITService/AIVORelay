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

function Get-CargoTargetDirUsers {
    param([Parameter(Mandatory = $true)][string]$CargoTargetDir)

    $processes = @(Get-CimInstance Win32_Process -ErrorAction Stop)
    $byId = @{}
    foreach ($process in $processes) {
        $byId[[int]$process.ProcessId] = $process
    }

    # This script and the shells/tools that started it are not a competing build.
    $excluded = @{}
    $ancestorId = $PID
    while ($byId.ContainsKey($ancestorId) -and -not $excluded.ContainsKey($ancestorId)) {
        $excluded[$ancestorId] = $true
        $ancestorId = [int]$byId[$ancestorId].ParentProcessId
    }

    # Only builds that write into or run from the same target dir conflict: Cargo
    # would wait on its lock or rebuild shared artifacts with different features.
    # Unrelated Bun/Cargo work, including dev builds in another target dir, is fine.
    $root = $ExecutionContext.SessionState.Path.GetUnresolvedProviderPathFromPSPath($CargoTargetDir).Replace("/", "\").TrimEnd("\")
    $mention = "(?i)(?<!\w)" + [regex]::Escape($root) + "(?=[\\""'\s;,]|$)"
    $toolNames = "^(cargo|tauri|tauri-cli|rustc|rustdoc|clippy-driver|bun|node|MSBuild|cmake|cl|link|lld-link|ninja)(\.exe)?$"
    $owners = @{}
    foreach ($process in $processes) {
        $processId = [int]$process.ProcessId
        if ($excluded.ContainsKey($processId)) { continue }
        $path = "$($process.ExecutablePath)".Replace("/", "\")
        $commandLine = "$($process.CommandLine)".Replace("/", "\")
        if (-not ($path -match $mention -or ($process.Name -match $toolNames -and $commandLine -match $mention))) { continue }

        # Report the build that owns it (e.g. `cargo test`), not each rustc or test binary.
        $owner = $process
        $seen = @{ $processId = $true }
        while ($true) {
            $parentId = [int]$owner.ParentProcessId
            if ($seen.ContainsKey($parentId) -or $excluded.ContainsKey($parentId) -or -not $byId.ContainsKey($parentId)) { break }
            $parent = $byId[$parentId]
            if ($parent.CreationDate -gt $owner.CreationDate -or $parent.Name -notmatch $toolNames) { break }
            $seen[$parentId] = $true
            $owner = $parent
        }
        $owners[[int]$owner.ProcessId] = $owner
    }

    return @($owners.Values | Sort-Object ProcessId)
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

    $cargoTargetDir = Get-PreferredCargoTargetDir -PreferredPath $PreferredCargoTargetDir
    if (-not $SkipProcessCheck) {
        $conflicts = @(Get-CargoTargetDirUsers -CargoTargetDir $cargoTargetDir)
        if ($conflicts.Count -gt 0) {
            $details = ($conflicts | ForEach-Object {
                $commandLine = "$($_.CommandLine)"
                if ($commandLine.Length -gt 200) { $commandLine = $commandLine.Substring(0, 200) + "..." }
                "$($_.Name) $($_.ProcessId) $commandLine"
            }) -join "`n"
            throw "Another build is using the Cargo target dir $cargoTargetDir. Wait for it to finish, stop it, or use a different target dir.`n$details"
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

    $hasX64DevEnvironment =
        -not [string]::IsNullOrWhiteSpace($env:VSCMD_VER) -and
        $env:VSCMD_ARG_TGT_ARCH -eq "x64" -and
        $env:VSCMD_ARG_HOST_ARCH -eq "x64" -and
        -not [string]::IsNullOrWhiteSpace($env:VCToolsInstallDir)

    if (-not $hasX64DevEnvironment) {
        $devShellCandidates = @(
            (Join-Path $vsPath "Common7\Tools\Microsoft.VisualStudio.DevShell.dll"),
            (Join-Path $vsPath "Common7\Tools\vsdevshell\Microsoft.VisualStudio.DevShell.dll")
        )
        $devShellModule = $devShellCandidates |
            Where-Object { Test-Path -LiteralPath $_ -PathType Leaf } |
            Select-Object -First 1
        if (-not $devShellModule) {
            throw "Visual Studio Developer PowerShell module was not found under '$vsPath\Common7\Tools'."
        }

        try {
            Import-Module -Name $devShellModule -ErrorAction Stop
            Enter-VsDevShell `
                -VsInstallPath $vsPath `
                -SkipAutomaticLocation `
                -Arch amd64 `
                -HostArch amd64 `
                -DevCmdArguments "-no_logo" `
                -ErrorAction Stop
        } catch {
            throw "Failed to initialize the Visual Studio x64 build environment: $($_.Exception.Message)"
        }
    }

    Set-BindgenWindowsEnv

    $previousCargoTargetDir = $env:CARGO_TARGET_DIR
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
