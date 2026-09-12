# User dev-build functions

```powershell

# Ensures the Vulkan loader is available next to the Tauri build inputs.
function Ensure-AivoRelayVulkanDll {
  [CmdletBinding()]
  param(
    [Parameter(Mandatory)]
    [string]$TargetRoot
  )

  $targetDll = Join-Path $TargetRoot "src-tauri\vulkan-1.dll"
  if (Test-Path -LiteralPath $targetDll -PathType Leaf) {
    return
  }

  $copySource = $null
  if ($env:VULKAN_SDK) {
    $sdkDll = Join-Path $env:VULKAN_SDK "Bin\vulkan-1.dll"
    if (Test-Path -LiteralPath $sdkDll -PathType Leaf) {
      $copySource = $sdkDll
    }
  }

  if (-not $copySource) {
    $systemDll = "C:\Windows\System32\vulkan-1.dll"
    if (Test-Path -LiteralPath $systemDll -PathType Leaf) {
      $copySource = $systemDll
    }
  }

  if (-not $copySource) {
    throw "Could not find vulkan-1.dll in VULKAN_SDK or C:\Windows\System32."
  }

  Copy-Item -LiteralPath $copySource -Destination $targetDll -Force
}

# Checks that WebView2 can bind the requested loopback CDP port.
function Assert-AivoRelayTcpPortAvailable {
  [CmdletBinding()]
  param(
    [Parameter(Mandatory)]
    [ValidateRange(1, 65535)]
    [int]$Port
  )

  $listener = [System.Net.Sockets.TcpListener]::new(
    [System.Net.IPAddress]::Loopback,
    $Port
  )
  try {
    $listener.Start()
  }
  catch {
    throw "Playwright CDP port $Port is already in use. Close the stale AivoRelay instance or choose -PlaywrightPort <port>."
  }
  finally {
    $listener.Stop()
  }
}

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

  $cargoTargetDir = "Q:\t\d"
  $previousCargoTargetDir = $env:CARGO_TARGET_DIR
  $hadPreviousCargoTargetDir = Test-Path Env:CARGO_TARGET_DIR
  $previousPlaywrightPort = $env:PLAYWRIGHT_TAURI_REMOTE_DEBUGGING_PORT
  $hadPreviousPlaywrightPort = Test-Path Env:PLAYWRIGHT_TAURI_REMOTE_DEBUGGING_PORT
  $locationPushed = $false
  try {
    New-Item -ItemType Directory -Force -Path $cargoTargetDir | Out-Null
    $env:CARGO_TARGET_DIR = $cargoTargetDir

    if ($EnablePlaywright) {
      Assert-AivoRelayTcpPortAvailable -Port $PlaywrightPort
      $env:PLAYWRIGHT_TAURI_REMOTE_DEBUGGING_PORT = $PlaywrightPort.ToString()
      Write-Host "Playwright CDP enabled on port $PlaywrightPort." -ForegroundColor Cyan
      Write-Host "Verify from another shell: Test-AivoRelayPlaywright -PlaywrightPort $PlaywrightPort" -ForegroundColor DarkGray
    }

    Push-Location -LiteralPath $target
    $locationPushed = $true
    Write-Host "Using CARGO_TARGET_DIR=$cargoTargetDir for AIVORelay dev." -ForegroundColor DarkGray
    Write-Host "Starting 'bun x tauri dev' in $target" -ForegroundColor Green

    # Run interactively so output shows in current shell
    & bun x tauri dev
    if ($LASTEXITCODE -ne 0) {
      throw "bun x tauri dev failed with exit code $LASTEXITCODE"
    }
  }
  finally {
    if ($locationPushed) {
      Pop-Location
    }

    if ($hadPreviousPlaywrightPort) {
      $env:PLAYWRIGHT_TAURI_REMOTE_DEBUGGING_PORT = $previousPlaywrightPort
    } else {
      Remove-Item Env:PLAYWRIGHT_TAURI_REMOTE_DEBUGGING_PORT -ErrorAction SilentlyContinue
    }

    if ($hadPreviousCargoTargetDir) {
      $env:CARGO_TARGET_DIR = $previousCargoTargetDir
    } else {
      Remove-Item Env:CARGO_TARGET_DIR -ErrorAction SilentlyContinue
    }
  }
}

# Attaches Playwright to an already running AivoRelay dev window and verifies the UI root.
function Test-AivoRelayPlaywright {
  [CmdletBinding()]
  param(
    [ValidateRange(1, 65535)]
    [int]$PlaywrightPort = 9333,
    [string]$ScreenshotPath
  )

  $target = 'Q:\AIVORelay'
  $checkScript = Join-Path $target 'scripts\check-playwright-tauri.py'
  if (-not (Test-Path -LiteralPath $checkScript -PathType Leaf)) {
    throw "Playwright check script not found: $checkScript"
  }
  if (-not (Get-Command python -ErrorAction SilentlyContinue)) {
    throw "'python' not found in PATH."
  }

  $arguments = @($checkScript, '--port', $PlaywrightPort.ToString())
  if (-not [string]::IsNullOrWhiteSpace($ScreenshotPath)) {
    $arguments += @('--screenshot', $ScreenshotPath)
  }

  & python @arguments
  if ($LASTEXITCODE -ne 0) {
    throw "AivoRelay Playwright check failed with exit code $LASTEXITCODE"
  }
}

# Exercises the TTS voice gallery on both TTS pages and restores the original TTS settings.
function Test-AivoRelayTtsGallery {
  [CmdletBinding()]
  param(
    [ValidateRange(1, 65535)]
    [int]$PlaywrightPort = 9333
  )

  $target = 'Q:\AIVORelay'
  $checkScript = Join-Path $target 'scripts\check-playwright-tts-gallery.py'
  if (-not (Test-Path -LiteralPath $checkScript -PathType Leaf)) {
    throw "TTS voice-gallery Playwright check script not found: $checkScript"
  }
  if (-not (Get-Command python -ErrorAction SilentlyContinue)) {
    throw "'python' not found in PATH."
  }

  & python $checkScript '--port' $PlaywrightPort.ToString()
  if ($LASTEXITCODE -ne 0) {
    throw "AivoRelay TTS voice-gallery Playwright check failed with exit code $LASTEXITCODE"
  }
}



```
or alternatively:

```powershell
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
    Dev-AivoRelay -EnablePlaywright:$EnablePlaywright -PlaywrightPort $PlaywrightPort
    return
  }

  $cargoDir = Join-Path $target ".cargo"
  $cargoConfigPath = Join-Path $cargoDir "config.toml"
  $hadCargoConfig = Test-Path -LiteralPath $cargoConfigPath
  $originalCargoConfigBytes = if ($hadCargoConfig) {
    [System.IO.File]::ReadAllBytes($cargoConfigPath)
  } else {
    $null
  }
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
    $utf8NoBom = [System.Text.UTF8Encoding]::new($false)
    [System.IO.File]::WriteAllText(
      $cargoConfigPath,
      "$newCargoConfig`r`n",
      $utf8NoBom
    )

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
      [System.IO.File]::WriteAllBytes(
        $cargoConfigPath,
        $originalCargoConfigBytes
      )
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

# Run from a second PowerShell while the app is open:
Test-AivoRelayPlaywright
Test-AivoRelayPlaywright -PlaywrightPort 9334 -ScreenshotPath .\aivorelay.png
```

Behavior notes:

- default behavior is unchanged when `-EnablePlaywright` is omitted
- when enabled, the same visible `bun x tauri dev` instance exposes WebView2 CDP on the selected port
- the launcher rejects an already occupied CDP port before starting a long build
- the previous `PLAYWRIGHT_TAURI_REMOTE_DEBUGGING_PORT` environment value is restored after the dev session exits
- the previous `CARGO_TARGET_DIR` value is restored after the dev session exits
- Fast mode restores `.cargo/config.toml` byte-for-byte, including its original encoding and trailing newlines
- `Test-AivoRelayPlaywright` verifies the main React root through a real Playwright CDP connection and can capture a screenshot

Checked-in repo alternative for agents / no-profile shells:

```powershell
pwsh -NoProfile -File .\scripts\start-playwright-tauri-dev.ps1

# In a second shell:
python .\scripts\check-playwright-tauri.py
```

See also [[PLAYWRIGHT_TAURI_CONNECTION]].

## Developer-environment helper

```powershell
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
```
