# Shared session controller for Dev-AivoRelay and Fast-Dev-AivoRelay.
# Dot-source this file, then call Invoke-AivoRelayDevSession.
. "$PSScriptRoot\confirm-aivorelay-dev-launch.ps1"

function Get-AivoRelayProductionSnapshot {
    param([string]$TargetRoot, [string]$CargoTargetDir)

    $apps = @(Get-CimInstance Win32_Process -Filter "Name = 'AivoRelay.exe'" -ErrorAction Stop |
        Where-Object {
            Test-AivoRelayProductionPath -Path $_.ExecutablePath -TargetRoot $TargetRoot -CargoTargetDir $CargoTargetDir
        })
    if ($apps.Count -gt 1) {
        throw 'More than one production AivoRelay is running. Leave only the instance to restore.'
    }
    return $apps | Select-Object ProcessId, ExecutablePath, CreationDate
}

function Stop-AivoRelayProductionForDev {
    param($Production, [ref]$RestoreProduction)

    $instance = Get-Process -Id $Production.ProcessId -ErrorAction SilentlyContinue
    if (-not $instance) { return }
    try {
        # Acquire a handle before revalidating the snapshot, so PID reuse cannot
        # redirect a later Kill() to a different process.
        $null = $instance.Handle
        $current = Get-CimInstance Win32_Process -Filter "ProcessId = $($Production.ProcessId)" -ErrorAction Stop
        if (-not $current -or $current.CreationDate -ne $Production.CreationDate -or
            $current.ExecutablePath -ne $Production.ExecutablePath -or $instance.HasExited) { return }

        # Record the restore intent before stopping, including exceptional exits.
        $RestoreProduction.Value = $true
        Write-Host '[AivoRelay] SWITCH: production -> dev. Stopping production...' -ForegroundColor Yellow
        if ($instance.CloseMainWindow()) { $null = $instance.WaitForExit(300) }
        if (-not $instance.HasExited) {
            $instance.Kill($true)
            if (-not $instance.WaitForExit(5000)) { throw 'Production AivoRelay did not exit.' }
        }
    }
    catch {
        # It may exit naturally between discovering it and opening/stopping it.
        if (-not $instance.HasExited) { throw }
    }
    finally { $instance.Dispose() }
}

function Stop-AivoRelaySessionApps {
    param([string]$SessionDir)

    # Keep app records until session cleanup, including across Cargo restarts.
    # A held process handle and its start time protect against recycled PIDs.
    foreach ($recordFile in @(Get-ChildItem -LiteralPath $SessionDir -Filter 'app-*.json')) {
        $record = Get-Content -LiteralPath $recordFile.FullName -Raw | ConvertFrom-Json
        $instance = Get-Process -Id $record.ProcessId -ErrorAction SilentlyContinue
        if (-not $instance) { continue }
        try {
            $null = $instance.Handle
            if ($instance.HasExited -or $instance.StartTime.ToUniversalTime().Ticks -ne $record.StartTimeTicks) { continue }
            $instance.Kill($true)
            if (-not $instance.WaitForExit(5000)) { throw 'The dev app did not stop.' }
        }
        catch {
            # Kill(true) on the controller may already have stopped this app.
            if (-not $instance.HasExited) { throw }
        }
        finally { $instance.Dispose() }
    }
}

function Restore-AivoRelayProduction {
    param($Production)

    # A manually started app takes priority; never launch a second instance.
    if (Get-CimInstance Win32_Process -Filter "Name = 'AivoRelay.exe'" -ErrorAction Stop) {
        Write-Host '[AivoRelay] An AivoRelay instance is already running; skipping production restore.' -ForegroundColor Yellow
        return
    }
    if (-not (Test-Path -LiteralPath $Production.ExecutablePath -PathType Leaf)) {
        throw "Production executable is no longer available: $($Production.ExecutablePath)"
    }
    Write-Host '[AivoRelay] SWITCH: dev -> production. Starting production...' -ForegroundColor Cyan
    $startInfo = [Diagnostics.ProcessStartInfo]::new($Production.ExecutablePath)
    $startInfo.UseShellExecute = $false
    $startInfo.WorkingDirectory = Split-Path -Parent $Production.ExecutablePath
    # The production app must not inherit the dev-only CDP port/profile.
    $null = $startInfo.Environment.Remove('PLAYWRIGHT_TAURI_REMOTE_DEBUGGING_PORT')
    $null = $startInfo.Environment.Remove('AIVORELAY_DEV_SESSION_DIR')
    $instance = [Diagnostics.Process]::Start($startInfo)
    $instance.Dispose()
    Write-Host '[AivoRelay] Production launch requested.' -ForegroundColor Green
}

function Invoke-AivoRelayDevSession {
    [CmdletBinding()]
    param(
        [Parameter(Mandatory)][string]$TargetRoot,
        [string]$CargoTargetDir = 'Q:\t\d',
        [switch]$NoSwitch
    )

    $ErrorActionPreference = 'Stop'
    if ([Console]::IsInputRedirected -or -not [Environment]::UserInteractive) {
        throw 'Run the dev launcher in an interactive console so Ctrl+C and Escape can be handled.'
    }
    $production = $null
    if ($NoSwitch) {
        Confirm-AivoRelayLaunch -TargetRoot $TargetRoot -CargoTargetDir $CargoTargetDir -NoSwitch
    } else {
        $production = Get-AivoRelayProductionSnapshot -TargetRoot $TargetRoot -CargoTargetDir $CargoTargetDir
    }
    $runnerScript = Join-Path $PSScriptRoot 'run-aivorelay-dev-app.ps1'
    # Multiple installations can produce multiple matches. ProcessStartInfo
    # needs one executable path, using the first match in command lookup order.
    $bunPath = (Get-Command bun.exe -CommandType Application -ErrorAction Stop | Select-Object -First 1).Path
    $pwshPath = (Get-Command pwsh.exe -CommandType Application -ErrorAction Stop | Select-Object -First 1).Path
    $hostInfo = & rustc -vV
    if ($LASTEXITCODE -ne 0) { throw 'Could not determine the Rust host target.' }
    $hostLine = $hostInfo | Where-Object { $_ -like 'host: *' } | Select-Object -First 1
    if (-not $hostLine) { throw 'Rust did not report a host target.' }
    $rustTarget = if ($env:CARGO_BUILD_TARGET) { $env:CARGO_BUILD_TARGET } else { $hostLine.Substring(6).Trim() }
    if ($rustTarget -notmatch '^[A-Za-z0-9_-]+-windows-msvc$') {
        throw "The dev switch supports Windows MSVC targets, got: $rustTarget"
    }
    # Use a TOML array rather than a shell command string, preserving spaces.
    $runnerArgs = @($pwshPath, '-NoProfile', '-File', $runnerScript) | ConvertTo-Json -Compress
    $runnerConfig = "target.$rustTarget.runner=$runnerArgs"
    $sessionDir = Join-Path ([IO.Path]::GetTempPath()) ("aivorelay-dev-" + [guid]::NewGuid().ToString('N'))
    $previousSessionDir = $env:AIVORELAY_DEV_SESSION_DIR
    $hadSessionDir = Test-Path Env:AIVORELAY_DEV_SESSION_DIR
    $previousControlC = [Console]::TreatControlCAsInput
    $controller = $null
    $restoreProduction = $false
    $skipRestore = $false
    $cleanupSucceeded = $true
    try {
        New-Item -ItemType Directory -Path $sessionDir -ErrorAction Stop | Out-Null
        $env:AIVORELAY_DEV_SESSION_DIR = $sessionDir
        # Raw Ctrl+C is read here instead of terminating the PowerShell pipeline.
        [Console]::TreatControlCAsInput = $true
        $startInfo = [Diagnostics.ProcessStartInfo]::new($bunPath)
        $startInfo.UseShellExecute = $false
        # Only the supervisor reads console keys; child tools must not consume them.
        $startInfo.RedirectStandardInput = $true
        $startInfo.WorkingDirectory = $TargetRoot
        foreach ($argument in @('x', 'tauri', 'dev', '--', '--config', $runnerConfig)) {
            $startInfo.ArgumentList.Add($argument)
        }
        if ($NoSwitch) {
            Write-Host '[AivoRelay] NO SWITCH: automatic switching is disabled. Ctrl+C / Escape: stop dev only.' -ForegroundColor Yellow
        } else {
            Write-Host '[AivoRelay] Ctrl+C: stop dev and restore production. Escape: stop dev without restoring.' -ForegroundColor Cyan
        }
        if ($production) {
            Write-Host '[AivoRelay] Building dev; production stays running until the new EXE is ready.' -ForegroundColor Cyan
        } else {
            Write-Host '[AivoRelay] No production instance to restore after this dev session.' -ForegroundColor DarkGray
        }
        $controller = [Diagnostics.Process]::Start($startInfo)
        while (-not $controller.HasExited) {
            if ([Console]::KeyAvailable) {
                $key = [Console]::ReadKey($true)
                if ($key.Key -eq [ConsoleKey]::Escape) {
                    $skipRestore = $true
                    Write-Host '[AivoRelay] ESC: stopping dev without restoring production...' -ForegroundColor Yellow
                    break
                }
                if ($key.KeyChar -eq [char]3 -or
                    ($key.Key -eq [ConsoleKey]::C -and ($key.Modifiers -band [ConsoleModifiers]::Control))) {
                    Write-Host '[AivoRelay] CTRL+C: stopping dev...' -ForegroundColor Cyan
                    break
                }
            }
            foreach ($requestFile in @(Get-ChildItem -LiteralPath $sessionDir -Filter 'request-*.json')) {
                $request = Get-Content -LiteralPath $requestFile.FullName -Raw | ConvertFrom-Json
                $executable = [IO.Path]::GetFullPath($request.ExecutablePath)
                $targetPrefix = [IO.Path]::GetFullPath($CargoTargetDir).TrimEnd('\', '/') + '\'
                if ([IO.Path]::GetFileName($executable) -ine 'aivorelay.exe' -or
                    -not $executable.StartsWith($targetPrefix, [StringComparison]::OrdinalIgnoreCase)) {
                    throw "Unexpected executable requested the dev handoff: $executable"
                }
                if ($production -and -not $restoreProduction) {
                    Stop-AivoRelayProductionForDev -Production $production -RestoreProduction ([ref]$restoreProduction)
                }
                if ($NoSwitch -and (Get-CimInstance Win32_Process -Filter "Name = 'AivoRelay.exe'" -ErrorAction Stop)) {
                    throw 'Another AivoRelay instance appeared during compilation (-NoSwitch); dev app was not launched.'
                }
                Write-Host '[AivoRelay] Dev build ready. Launching dev app...' -ForegroundColor Green
                $replyName = $requestFile.BaseName.Replace('request-', 'ready-')
                [IO.File]::WriteAllText((Join-Path $sessionDir $replyName), '')
                Remove-Item -LiteralPath $requestFile.FullName -Force
            }
            Start-Sleep -Milliseconds 80
            $controller.Refresh()
        }
        if ($controller.HasExited -and $controller.ExitCode -ne 0) {
            throw "bun x tauri dev exited with code $($controller.ExitCode)"
        }
    }
    finally {
        try {
            if (Test-Path -LiteralPath $sessionDir) {
                [IO.File]::WriteAllText((Join-Path $sessionDir 'stopping'), '')
            }
            if ($controller -and -not $controller.HasExited) {
                $controller.Kill($true)
                if (-not $controller.WaitForExit(10000)) { throw 'Dev process tree did not stop.' }
            }
            if (Test-Path -LiteralPath $sessionDir) {
                Stop-AivoRelaySessionApps -SessionDir $sessionDir
            }
        }
        catch {
            $cleanupSucceeded = $false
            Write-Warning "Dev shutdown failed; production will not be started over an active dev session: $_"
        }
        finally {
            [Console]::TreatControlCAsInput = $previousControlC
            if ($hadSessionDir) { $env:AIVORELAY_DEV_SESSION_DIR = $previousSessionDir }
            else { Remove-Item Env:AIVORELAY_DEV_SESSION_DIR -ErrorAction SilentlyContinue }
            if (Test-Path -LiteralPath $sessionDir) {
                try {
                    # Delete only this invocation's files, never recurse through temp.
                    Get-ChildItem -LiteralPath $sessionDir -File | Remove-Item -Force -ErrorAction Stop
                    Remove-Item -LiteralPath $sessionDir -ErrorAction Stop
                }
                catch { Write-Warning "Could not remove dev session files: $_" }
            }
            if ($controller) { $controller.Dispose() }
        }
        if ($restoreProduction -and -not $skipRestore -and $cleanupSucceeded) {
            Restore-AivoRelayProduction -Production $production
        } elseif ($skipRestore -and $restoreProduction -and $cleanupSucceeded) {
            Write-Host '[AivoRelay] Dev stopped. Production remains off (Escape).' -ForegroundColor Yellow
        } elseif ($cleanupSucceeded) {
            Write-Host '[AivoRelay] Dev stopped. No production restart needed.' -ForegroundColor DarkGray
        }
    }
}
