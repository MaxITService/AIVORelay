param(
    [switch]$DoBuild,
    [switch]$DoDebugBuild,
    [switch]$DoDev,
    [string]$CudaPath = "C:\Program Files\NVIDIA GPU Computing Toolkit\CUDA\v12.4",
    [string]$DependencyRoot = "C:\Code\AIVORelay-deps"
)

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

$selectedModes = @(@($DoBuild, $DoDebugBuild, $DoDev) | Where-Object { $_ })

if ($selectedModes.Count -gt 1) {
    throw "Use only one of -DoBuild, -DoDebugBuild, or -DoDev."
}

if (-not $DoBuild -and -not $DoDebugBuild -and -not $DoDev) {
    $DoBuild = $true
}

$repoRoot = Split-Path -Parent $MyInvocation.MyCommand.Path
$llvmBin = "C:\Program Files\LLVM\bin"
$clangExe = Join-Path $llvmBin "clang.exe"
$vswhere = Join-Path ${env:ProgramFiles(x86)} "Microsoft Visual Studio\Installer\vswhere.exe"

function Set-ProcessEnv([string]$Name, [string]$Value) {
    Set-Item -Path "Env:$Name" -Value $Value
    [Environment]::SetEnvironmentVariable($Name, $Value, "Process")
}

function Import-VsDevEnvironment {
    if (-not (Test-Path $vswhere)) {
        throw "vswhere.exe not found at $vswhere"
    }

    $instances = & $vswhere -products * -format json | ConvertFrom-Json
    $vsPath = $null

    foreach ($instance in $instances) {
        $candidate = $instance.installationPath
        if (-not $candidate) {
            continue
        }

        if (Test-Path (Join-Path $candidate "VC\Tools\MSVC")) {
            $vsPath = $candidate
            break
        }
    }

    if (-not $vsPath) {
        $communityPath = "C:\Program Files\Microsoft Visual Studio\2022\Community"
        if (Test-Path (Join-Path $communityPath "VC\Tools\MSVC")) {
            $vsPath = $communityPath
        }
    }

    if (-not $vsPath) {
        throw "A Visual Studio 2022 instance with MSVC tools was not found."
    }

    $devCmd = Join-Path $vsPath "Common7\Tools\VsDevCmd.bat"
    if (-not (Test-Path $devCmd)) {
        throw "VsDevCmd.bat not found at $devCmd"
    }

    cmd /c "`"$devCmd`" -arch=x64 && set" |
        Where-Object { $_ -match '^(.+?)=(.*)$' } |
        ForEach-Object { Set-Item -Path "Env:$($Matches[1])" -Value $Matches[2] }
}

function Set-BindgenEnv {
    if (-not (Test-Path $clangExe)) {
        throw "clang.exe not found at $clangExe"
    }

    $resourceDir = (& $clangExe --print-resource-dir).Trim()
    if (-not $resourceDir) {
        throw "Unable to determine clang resource dir."
    }

    $winSdkDir = $env:WindowsSdkDir
    $winSdkVer = $env:WindowsSDKVersion.TrimEnd("\")
    $vcToolsDir = $env:VCToolsInstallDir

    if (-not $vcToolsDir -and $env:VSINSTALLDIR) {
        $msvcRoot = Join-Path $env:VSINSTALLDIR "VC\Tools\MSVC"
        if (Test-Path $msvcRoot) {
            $latestMsvc = Get-ChildItem -Directory $msvcRoot | Sort-Object Name -Descending | Select-Object -First 1
            if ($latestMsvc) {
                $vcToolsDir = $latestMsvc.FullName
                Set-ProcessEnv "VCToolsInstallDir" $vcToolsDir
            }
        }
    }

    if (-not $winSdkDir -or -not $winSdkVer -or -not $vcToolsDir) {
        throw "VS/Windows SDK environment is incomplete after VsDevCmd."
    }

    $vcInclude = Join-Path $vcToolsDir "include"
    $ucrt = Join-Path $winSdkDir "Include\$winSdkVer\ucrt"
    $um = Join-Path $winSdkDir "Include\$winSdkVer\um"
    $shared = Join-Path $winSdkDir "Include\$winSdkVer\shared"
    $clangBuiltinInclude = Join-Path $resourceDir "include"

    $includeValue = @($vcInclude, $ucrt, $um, $shared) -join ";"
    Set-ProcessEnv "INCLUDE" $includeValue
    Set-ProcessEnv "LIBCLANG_PATH" $llvmBin

    $clangArgs = @(
        "--target=x86_64-pc-windows-msvc"
        "-resource-dir `"$resourceDir`""
        "-I`"$clangBuiltinInclude`""
        "-I`"$vcInclude`""
        "-I`"$ucrt`""
        "-I`"$um`""
        "-I`"$shared`""
        "-fms-compatibility"
        "-fms-extensions"
        "-fms-compatibility-version=19"
        "-x c++"
        "-std=c++14"
    ) -join " "

    Set-ProcessEnv "BINDGEN_EXTRA_CLANG_ARGS" $clangArgs
    [Environment]::SetEnvironmentVariable(
        "BINDGEN_EXTRA_CLANG_ARGS_x86_64-pc-windows-msvc",
        $clangArgs,
        "Process"
    )
}

function Set-CudaEnv {
    if (-not (Test-Path $CudaPath)) {
        throw "CUDA 12.4 was not found at $CudaPath"
    }

    Set-ProcessEnv "CUDA_PATH" $CudaPath
    Set-ProcessEnv "CMAKE_GENERATOR" "Ninja"
    Set-ProcessEnv "CARGO_TARGET_DIR" "C:/aivorelay-cuda"

    $env:PATH = "$CudaPath\bin;$CudaPath\libnvvp;$llvmBin;$env:PATH"
    [Environment]::SetEnvironmentVariable("PATH", $env:PATH, "Process")

    Remove-Item Env:WHISPER_DONT_GENERATE_BINDINGS -ErrorAction SilentlyContinue
}

function Assert-LocalForks {
    $transcribeRoot = Join-Path $DependencyRoot "AIVORelay-dep-transcribe-rs"
    $whisperRoot = Join-Path $DependencyRoot "AIVORelay-dep-whisper-rs"
    $requiredPaths = @(
        (Join-Path $transcribeRoot "Cargo.toml"),
        (Join-Path $whisperRoot "Cargo.toml"),
        (Join-Path $whisperRoot "sys\build.rs"),
        (Join-Path $whisperRoot "sys\whisper.cpp\include\whisper.h")
    )

    foreach ($path in $requiredPaths) {
        if (-not (Test-Path $path)) {
            if ($path -like "*sys\whisper.cpp\include\whisper.h") {
                throw "Required whisper.cpp headers are missing: $path . Ensure the AIVORelay-dep-whisper-rs checkout includes its sys/whisper.cpp submodule."
            }
            throw "Required local CUDA dependency is missing: $path"
        }
    }
}

function Sync-CargoPatchPaths {
    $cargoTomlPath = Join-Path $repoRoot "src-tauri\Cargo.toml"
    $transcribePath = (Join-Path $DependencyRoot "AIVORelay-dep-transcribe-rs").Replace("\", "/")
    $whisperPath = (Join-Path $DependencyRoot "AIVORelay-dep-whisper-rs").Replace("\", "/")
    $whisperSysPath = "$whisperPath/sys"

    $content = Get-Content $cargoTomlPath -Raw
    $updated = $content
    $updated = [regex]::Replace($updated, 'transcribe-rs = \{ path = "[^"]+" \}', "transcribe-rs = { path = `"$transcribePath`" }")
    $updated = [regex]::Replace($updated, 'whisper-rs = \{ path = "[^"]+" \}', "whisper-rs = { path = `"$whisperPath`" }")
    $updated = [regex]::Replace($updated, 'whisper-rs-sys = \{ path = "[^"]+" \}', "whisper-rs-sys = { path = `"$whisperSysPath`" }")

    if ($updated -ne $content) {
        Set-Content -Path $cargoTomlPath -Value $updated -NoNewline
    }
}

function Invoke-LoggedNativeCommand {
    param(
        [Parameter(Mandatory = $true)][string]$FilePath,
        [Parameter(Mandatory = $true)][string[]]$ArgumentList,
        [Parameter(Mandatory = $true)][string]$LogPrefix
    )

    $combinedLog = Join-Path $repoRoot "$LogPrefix.log"

    if (Test-Path $combinedLog) { Remove-Item $combinedLog -Force }

    Push-Location $repoRoot
    try {
        & $FilePath @ArgumentList 2>&1 | Tee-Object -FilePath $combinedLog
        $exitCode = $LASTEXITCODE
    }
    finally {
        Pop-Location
    }

    if ($exitCode -ne 0) {
        throw "$FilePath $($ArgumentList -join ' ') failed with exit code $exitCode"
    }
}

Push-Location $repoRoot
try {
    Assert-LocalForks
    Sync-CargoPatchPaths
    Import-VsDevEnvironment
    Set-BindgenEnv
    Set-CudaEnv

    Write-Host "--- START TASK ---"
    Invoke-LoggedNativeCommand -FilePath "bun" -ArgumentList @("install") -LogPrefix "bun-install"
    Write-Host "--- END TASK ---"

    if ($DoDev) {
        Write-Host "--- START TASK ---"
        Invoke-LoggedNativeCommand -FilePath "bun" -ArgumentList @("run", "tauri", "dev", "--release") -LogPrefix "tauri-dev"
        Write-Host "--- END TASK ---"
        exit 0
    }

    if ($DoDebugBuild) {
        Write-Host "--- START TASK ---"
        Invoke-LoggedNativeCommand -FilePath "bun" -ArgumentList @("run", "tauri", "build", "--debug", "--no-sign", "--no-bundle") -LogPrefix "tauri-build-debug"
        Write-Host "--- END TASK ---"
        exit 0
    }

    Write-Host "--- START TASK ---"
    Invoke-LoggedNativeCommand -FilePath "bun" -ArgumentList @("run", "tauri", "build", "--no-sign", "--no-bundle") -LogPrefix "tauri-build"
    Write-Host "--- END TASK ---"
    exit 0
}
finally {
    Pop-Location
}
