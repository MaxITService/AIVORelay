# Shared preflight for interactive AivoRelay development launchers.
function Get-AivoRelayLaunchProcesses {
    [CmdletBinding()]
    param(
        [Parameter(Mandatory)][string]$TargetRoot,
        [string]$CargoTargetDir = 'Q:\t\d'
    )

    $processes = @(Get-CimInstance Win32_Process -ErrorAction Stop)
    $byId = @{}
    foreach ($process in $processes) {
        $byId[[int]$process.ProcessId] = $process
    }

    # Never include this launcher or the interactive shell that owns it.
    $excluded = @{}
    $ancestorId = $PID
    while ($byId.ContainsKey($ancestorId) -and -not $excluded.ContainsKey($ancestorId)) {
        $excluded[$ancestorId] = $true
        $ancestorId = [int]$byId[$ancestorId].ParentProcessId
    }

    $markers = @(
        ([System.IO.Path]::GetFullPath($TargetRoot).TrimEnd('\', '/') + '\'),
        ([System.IO.Path]::GetFullPath($CargoTargetDir).TrimEnd('\', '/') + '\')
    )
    $toolNames = '^(cargo|tauri|tauri-cli|rustc|bun|node|vite|esbuild|MSBuild|cmake|cl|link|lld-link|ninja)(\.exe)?$'
    $selected = @{}
    foreach ($process in $processes) {
        $processId = [int]$process.ProcessId
        if ($excluded.ContainsKey($processId)) { continue }
        $isApp = $process.Name -match '^aivorelay(\.exe)?$'
        $hasMarker = $false
        if ($process.Name -match $toolNames) {
            $description = ("$($process.ExecutablePath) $($process.CommandLine)").Replace('/', '\')
            foreach ($marker in $markers) {
                if ($description.IndexOf($marker, [StringComparison]::OrdinalIgnoreCase) -ge 0) {
                    $hasMarker = $true
                    break
                }
            }
        }
        if ($isApp -or $hasMarker) {
            $selected[$processId] = $process
        }
    }

    # Include the identified dev controller and its descendants, while leaving
    # PowerShell consoles and unrelated Bun/Node/build sessions alone.
    $changed = $true
    while ($changed) {
        $changed = $false
        foreach ($process in @($selected.Values)) {
            $parentId = [int]$process.ParentProcessId
            if ($excluded.ContainsKey($parentId) -or $selected.ContainsKey($parentId) -or -not $byId.ContainsKey($parentId)) { continue }
            $parent = $byId[$parentId]
            if ($parent.CreationDate -gt $process.CreationDate) { continue }
            if ($parent.Name -match $toolNames -or ($parent.Name -match '^cmd(\.exe)?$' -and $parent.CommandLine -match '(?i)\s/c\s')) {
                $selected[$parentId] = $parent
                $changed = $true
            }
        }
        foreach ($process in $processes) {
            $processId = [int]$process.ProcessId
            $parentId = [int]$process.ParentProcessId
            if ($excluded.ContainsKey($processId) -or $selected.ContainsKey($processId) -or -not $selected.ContainsKey($parentId)) { continue }
            if ($selected[$parentId].CreationDate -gt $process.CreationDate) { continue }
            $selected[$processId] = $process
            $changed = $true
        }
    }

    foreach ($process in $selected.Values) {
        $depth = 0
        $parentId = [int]$process.ParentProcessId
        $seen = @{}
        while ($selected.ContainsKey($parentId) -and -not $seen.ContainsKey($parentId)) {
            $seen[$parentId] = $true
            $depth++
            $parentId = [int]$selected[$parentId].ParentProcessId
        }
        [pscustomobject]@{
            ProcessId = [int]$process.ProcessId
            Name = $process.Name
            ExecutablePath = $process.ExecutablePath
            CreationDate = $process.CreationDate
            Depth = $depth
        }
    }
}

function Assert-AivoRelayLaunchPorts {
    [CmdletBinding()]
    param([int[]]$Ports)

    foreach ($port in ($Ports | Select-Object -Unique)) {
        $listener = [System.Net.Sockets.TcpListener]::new([System.Net.IPAddress]::Loopback, $port)
        try {
            $listener.Start()
        }
        catch {
            throw "Dev port $port is still in use by another process. Close its owner or select another port before launching AivoRelay."
        }
        finally {
            $listener.Stop()
        }
    }
}

function Assert-AivoRelayLaunchConfirmation {
    if (-not [Environment]::UserInteractive -or [Console]::IsInputRedirected) {
        throw 'Close the listed AivoRelay processes, or run the launcher in an interactive PowerShell to confirm with Enter.'
    }
    $answer = Read-Host 'Press Enter to stop these processes and continue; type anything to cancel'
    if ($answer -ne '') {
        throw 'AivoRelay launch cancelled; the existing processes were left running.'
    }
}

function Confirm-AivoRelayLaunch {
    [CmdletBinding()]
    param(
        [Parameter(Mandatory)][string]$TargetRoot,
        [string]$CargoTargetDir = 'Q:\t\d',
        [int[]]$Ports = @(1420)
    )

    $lookup = @{ TargetRoot = $TargetRoot; CargoTargetDir = $CargoTargetDir }
    $running = @(Get-AivoRelayLaunchProcesses @lookup)
    if ($running.Count -eq 0) {
        Assert-AivoRelayLaunchPorts -Ports $Ports
        return
    }

    Write-Host 'An AivoRelay app or development session is already running:' -ForegroundColor Yellow
    $running | Sort-Object Depth, ProcessId |
        Select-Object Name, ProcessId, ExecutablePath | Format-Table -AutoSize | Out-Host
    Assert-AivoRelayLaunchConfirmation

    # Stop controllers first so they cannot respawn the app/server. Validate
    # creation times immediately before stopping, in case a PID was reused.
    foreach ($process in ($running | Sort-Object Depth, ProcessId)) {
        $current = Get-CimInstance Win32_Process -Filter "ProcessId = $($process.ProcessId)" -ErrorAction Stop
        if (-not $current -or $current.CreationDate -ne $process.CreationDate) { continue }
        try {
            $instance = Get-Process -Id $process.ProcessId -ErrorAction Stop
            if ($process.Name -match '^aivorelay(\.exe)?$' -and $instance.CloseMainWindow()) {
                $null = $instance.WaitForExit(1500)
            }
            if (-not $instance.HasExited) {
                Stop-Process -InputObject $instance -Force -ErrorAction Stop
                if (-not $instance.WaitForExit(5000)) {
                    throw 'The process did not exit.'
                }
            }
        }
        catch {
            if (Get-CimInstance Win32_Process -Filter "ProcessId = $($process.ProcessId)" -ErrorAction Stop |
                Where-Object { $_.CreationDate -eq $process.CreationDate }) {
                throw "Could not stop $($process.Name) [$($process.ProcessId)]: $($_.Exception.Message)"
            }
        }
    }

    # Allow the previous interactive launcher's finally block to restore its
    # temporary Cargo config before Fast-Dev snapshots that file again.
    Start-Sleep -Milliseconds 500
    $remaining = @(Get-AivoRelayLaunchProcesses @lookup)
    if ($remaining.Count -gt 0) {
        throw 'An AivoRelay process is still running or was restarted. Close that session before launching again.'
    }
    Assert-AivoRelayLaunchPorts -Ports $Ports
    Write-Host 'Previous AivoRelay processes stopped. Continuing launch...' -ForegroundColor Green
}
