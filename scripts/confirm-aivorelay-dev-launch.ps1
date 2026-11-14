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

function Test-AivoRelayProductionPath {
    param([string]$Path, [string]$TargetRoot, [string]$CargoTargetDir)

    if (-not $Path -or $Path -match '(?i)\\debug\\') { return $false }
    $fullPath = [IO.Path]::GetFullPath($Path)
    foreach ($root in @($TargetRoot, $CargoTargetDir)) {
        $prefix = [IO.Path]::GetFullPath($root).TrimEnd('\', '/') + '\'
        if ($fullPath.StartsWith($prefix, [StringComparison]::OrdinalIgnoreCase)) { return $false }
    }
    return $true
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

function Confirm-AivoRelayLaunch {
    [CmdletBinding()]
    param(
        [Parameter(Mandatory)][string]$TargetRoot,
        [string]$CargoTargetDir = 'Q:\t\d',
        [int[]]$Ports = @(1420),
        [switch]$NoSwitch
    )

    # The switching launcher manages production itself. Only -NoSwitch needs
    # the old process lookup, and it must never prompt or terminate processes.
    if ($NoSwitch) {
        $running = @(Get-AivoRelayLaunchProcesses -TargetRoot $TargetRoot -CargoTargetDir $CargoTargetDir)
        if ($running.Count -gt 0) {
            Write-Host 'An AivoRelay app or development session is already running:' -ForegroundColor Yellow
            $running | Sort-Object Depth, ProcessId |
                Select-Object Name, ProcessId, ExecutablePath | Format-Table -AutoSize | Out-Host
            throw 'AivoRelay launch cancelled (-NoSwitch): close the existing app/dev session first. No processes were stopped.'
        }
    }
    Assert-AivoRelayLaunchPorts -Ports $Ports
}
