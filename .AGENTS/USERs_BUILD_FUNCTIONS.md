#User uses 2 functions to build dev build:

```

# Dev-AivoRelay
# Runs Get-Dev, prepares the Windows bindgen/Vulkan environment, shortens Cargo target output, then starts the Tauri dev server via bun.
# Optional: -EnablePlaywright exposes the same visible dev window over WebView2 CDP.
function Dev-AivoRelay {
  [CmdletBinding()]
  param(
    [switch]$EnablePlaywright,
    [ValidateRange(1, 65535)]
    [int]$PlaywrightPort = 9333
  )

  # Run developer environment launcher if available
  if (Get-Command Get-Dev -ErrorAction SilentlyContinue) {
    Write-Host "Running Get-Dev..." -ForegroundColor Cyan
    Get-Dev
  }
  else {
    Write-Warning "Get-Dev not found; continuing without it."
  }

  $target = 'Q:\AIVORelay'
  if (-not (Test-Path -LiteralPath $target)) {
    Throw "Target folder not found: $target"
  }

  if (-not (Get-Command bun -ErrorAction SilentlyContinue)) {
    throw "'bun' not found in PATH; ensure bun is installed and available."
  }

  Set-AivoRelayBindgenWindowsEnv
  Ensure-AivoRelayVulkanDll -TargetRoot $target

  $cargoTargetDir = "Q:\t\aivorelay-dev"
  $previousPlaywrightPort = $env:PLAYWRIGHT_TAURI_REMOTE_DEBUGGING_PORT
  $hadPreviousPlaywrightPort = Test-Path Env:PLAYWRIGHT_TAURI_REMOTE_DEBUGGING_PORT
  try {
    New-Item -ItemType Directory -Force -Path $cargoTargetDir | Out-Null
    $env:CARGO_TARGET_DIR = $cargoTargetDir

    if ($EnablePlaywright) {
      $env:PLAYWRIGHT_TAURI_REMOTE_DEBUGGING_PORT = $PlaywrightPort.ToString()
      Write-Host "Playwright CDP enabled on port $PlaywrightPort." -ForegroundColor Cyan
    }

    Push-Location -LiteralPath $target
    Write-Host "Using CARGO_TARGET_DIR=$cargoTargetDir for AIVORelay dev." -ForegroundColor DarkGray
    Write-Host "Starting 'bun x tauri dev' in $target" -ForegroundColor Green

    # Run interactively so output shows in current shell
    & bun x tauri dev
  }
  finally {
    if ($hadPreviousPlaywrightPort) {
      $env:PLAYWRIGHT_TAURI_REMOTE_DEBUGGING_PORT = $previousPlaywrightPort
    } else {
      Remove-Item Env:PLAYWRIGHT_TAURI_REMOTE_DEBUGGING_PORT -ErrorAction SilentlyContinue
    }

    Pop-Location
  }
}



```
or alternatively:

```
# Fast-Dev-AivoRelay
# Runs Dev-AivoRelay with the fastest safe local config found so far:
# - lld-link on Windows MSVC
# - limited debuginfo for the workspace crate
# - dependency debuginfo disabled via temporary Cargo config override
# The original repo config and environment variables are restored when the dev session exits.
function Fast-Dev-AivoRelay {
  [CmdletBinding()]
  param(
    [switch]$EnablePlaywright,
    [ValidateRange(1, 65535)]
    [int]$PlaywrightPort = 9333
  )

  $target = 'Q:\AIVORelay'
  if (-not (Test-Path -LiteralPath $target)) {
    Throw "Target folder not found: $target"
  }

  $lldLinkPath = "C:\Program Files\LLVM\bin\lld-link.exe"
  if (-not (Test-Path -LiteralPath $lldLinkPath)) {
    Write-Warning "lld-link.exe not found at $lldLinkPath. Falling back to Dev-AivoRelay."
    Dev-AivoRelay
    return
  }

  $cargoDir = Join-Path $target ".cargo"
  $cargoConfigPath = Join-Path $cargoDir "config.toml"
  $hadCargoConfig = Test-Path -LiteralPath $cargoConfigPath
  $originalCargoConfig = if ($hadCargoConfig) {
    Get-Content -LiteralPath $cargoConfigPath -Raw
  } else {
    $null
  }

  $previousLinker = $env:CARGO_TARGET_X86_64_PC_WINDOWS_MSVC_LINKER
  $hadPreviousLinker = Test-Path Env:CARGO_TARGET_X86_64_PC_WINDOWS_MSVC_LINKER
  $previousDebug = $env:CARGO_PROFILE_DEV_DEBUG
  $hadPreviousDebug = Test-Path Env:CARGO_PROFILE_DEV_DEBUG

  try {
    New-Item -ItemType Directory -Force -Path $cargoDir | Out-Null

    $configBase = if ($originalCargoConfig) { $originalCargoConfig.TrimEnd() } else { "" }
    $fastConfig = @'
[profile.dev.package."*"]
debug = false
'@
    $newCargoConfig = if ([string]::IsNullOrWhiteSpace($configBase)) {
      $fastConfig.Trim()
    } else {
      "$configBase`r`n`r`n$($fastConfig.Trim())"
    }
    Set-Content -LiteralPath $cargoConfigPath -Value $newCargoConfig

    $env:CARGO_TARGET_X86_64_PC_WINDOWS_MSVC_LINKER = $lldLinkPath
    $env:CARGO_PROFILE_DEV_DEBUG = "limited"

    Write-Host "Fast dev config enabled:" -ForegroundColor Cyan
    Write-Host "  linker = lld-link.exe" -ForegroundColor DarkGray
    Write-Host "  profile.dev.debug = limited" -ForegroundColor DarkGray
    Write-Host "  profile.dev.package.\"*\".debug = false" -ForegroundColor DarkGray

    Dev-AivoRelay -EnablePlaywright:$EnablePlaywright -PlaywrightPort $PlaywrightPort
  }
  finally {
    if ($hadCargoConfig) {
      Set-Content -LiteralPath $cargoConfigPath -Value $originalCargoConfig
    } elseif (Test-Path -LiteralPath $cargoConfigPath) {
      Remove-Item -LiteralPath $cargoConfigPath -Force
    }

    if ($hadPreviousLinker) {
      $env:CARGO_TARGET_X86_64_PC_WINDOWS_MSVC_LINKER = $previousLinker
    } else {
      Remove-Item Env:CARGO_TARGET_X86_64_PC_WINDOWS_MSVC_LINKER -ErrorAction SilentlyContinue
    }

    if ($hadPreviousDebug) {
      $env:CARGO_PROFILE_DEV_DEBUG = $previousDebug
    } else {
      Remove-Item Env:CARGO_PROFILE_DEV_DEBUG -ErrorAction SilentlyContinue
    }
  }
}
```

Playwright-enabled launch examples:

```powershell
Dev-AivoRelay -EnablePlaywright
Fast-Dev-AivoRelay -EnablePlaywright
Fast-Dev-AivoRelay -EnablePlaywright -PlaywrightPort 9334
```

Behavior notes:

- default behavior is unchanged when `-EnablePlaywright` is omitted
- when enabled, the same visible `bun x tauri dev` instance exposes WebView2 CDP on the selected port
- the previous `PLAYWRIGHT_TAURI_REMOTE_DEBUGGING_PORT` environment value is restored after the dev session exits

Checked-in repo alternative for agents / no-profile shells:

```powershell
pwsh -NoProfile -File .\scripts\start-playwright-tauri-dev.ps1
```

See also [[PLAYWRIGHT_TAURI_CONNECTION]].

## helper function:

```
function Get-Dev {
  [CmdletBinding()]
  param(
    [switch]$Force
  )

  if (-not $Force -and $env:VSCMD_VER) {
    Write-Host "VS dev environment already loaded: $env:VSCMD_VER" -ForegroundColor DarkGray
    return
  }

  $vsWhere = Join-Path ${env:ProgramFiles(x86)} "Microsoft Visual Studio\Installer\vswhere.exe"
  if (-not (Test-Path -LiteralPath $vsWhere)) {
    throw "vswhere.exe not found at: $vsWhere"
  }

  $vsPath = & $vsWhere -latest -products * -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath
  if (-not $vsPath) {
    throw "Visual Studio installation not found."
  }

  $vsDevCmd = Join-Path $vsPath "Common7\Tools\VsDevCmd.bat"
  if (-not (Test-Path -LiteralPath $vsDevCmd)) {
    throw "VsDevCmd.bat not found at: $vsDevCmd"
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

  Write-Host "Developer environment loaded from: $vsPath" -ForegroundColor Green
}


function Ensure-AivoRelayVulkanDll([string]$TargetRoot) {
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

  if ($copySource) {
    Copy-Item -LiteralPath $copySource -Destination $targetDll -Force
  }
}

```
