$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
. "$PSScriptRoot\confirm-aivorelay-dev-launch.ps1"

$script:processFixture = @()
$script:checks = 0
$script:forbidLookup = $false
function Get-CimInstance {
    param([string]$ClassName, [string]$Filter, [string]$ErrorAction)
    if ($script:forbidLookup) { throw 'Switching mode must not perform the old process preflight.' }
    if ($Filter) { throw 'Preflight must never inspect processes for termination.' }
    return $script:processFixture
}
function Stop-Process { throw 'Preflight must never stop a real process.' }
function Get-Process { throw 'Preflight must never access a process for termination.' }
function Read-Host { throw 'A non-interactive launch must not prompt or stop a process.' }
function Assert-Equal {
    param($Actual, $Expected, [string]$Message)
    if (($Actual -join ',') -ne ($Expected -join ',')) {
        throw "$Message`: expected [$($Expected -join ',')], got [$($Actual -join ',')]."
    }
    $script:checks++
}
function New-ProcessFixture {
    param([int]$Id, [int]$Parent, [string]$Name, [string]$Command, [int]$Created = 0, [string]$Path = '')
    [pscustomobject]@{
        ProcessId = $Id
        ParentProcessId = $Parent
        Name = $Name
        CommandLine = $Command
        ExecutablePath = $Path
        CreationDate = [datetime]'2026-10-09T00:00:00Z' + [timespan]::FromSeconds($Created)
    }
}
function Get-SelectedIds {
    @(Get-AivoRelayLaunchProcesses -TargetRoot 'Q:\AIVORelay' | Sort-Object ProcessId | ForEach-Object { $_.ProcessId })
}

# Include the native watcher and all Vite/app children even when their own
# command lines lack the checkout path. Leave unrelated tools/consoles alone.
$script:processFixture = @(
    (New-ProcessFixture 100 0 pwsh.exe ''),
    (New-ProcessFixture 101 100 bun.exe 'bun x tauri dev' 1),
    (New-ProcessFixture 102 101 node.exe 'node Q:/AIVORelay/node_modules/@tauri-apps/cli/tauri.js dev' 2),
    (New-ProcessFixture 103 102 tauri.exe 'tauri dev' 3),
    (New-ProcessFixture 104 103 node.exe 'node vite' 4),
    (New-ProcessFixture 105 104 esbuild.exe 'esbuild --service' 5),
    (New-ProcessFixture 106 103 aivorelay.exe '' 4),
    (New-ProcessFixture 200 0 bun.exe 'bun C:\DesktopApp\orchestrator.js'),
    (New-ProcessFixture 201 200 node.exe 'node unrelated.js' 1),
    (New-ProcessFixture 202 0 cargo.exe 'cargo check --manifest-path Q:\AnotherProject\Cargo.toml'),
    (New-ProcessFixture 203 0 MSBuild.exe '/nodemode:1 /nodeReuse:true')
)
Assert-Equal (Get-SelectedIds) @(101, 102, 103, 104, 105, 106) 'Dev process tree selection'
$depths = @(Get-AivoRelayLaunchProcesses -TargetRoot 'Q:\AIVORelay' | Sort-Object ProcessId | ForEach-Object { $_.Depth })
Assert-Equal $depths @(0, 1, 2, 3, 4, 3) 'Controller/child depth ordering'

$script:processFixture = @(
    (New-ProcessFixture 210 0 node.exe 'node Q:\AIVORelay-store\node_modules\vite.js'),
    (New-ProcessFixture 211 0 node.exe 'node Q:\AIVORelay-cuda\node_modules\vite.js'),
    (New-ProcessFixture 212 0 node.exe 'node Q:\AIVORelay\node_modules\vite.js'),
    (New-ProcessFixture 213 0 aivorelay.exe '' 0 'C:\Program Files\AivoRelay\aivorelay.exe'),
    (New-ProcessFixture 214 0 cargo.exe 'cargo build' 0 'Q:\t\d\tools\cargo.exe'),
    (New-ProcessFixture 215 0 node.exe 'node Q:\t\different\vite.js')
)
Assert-Equal (Get-SelectedIds) @(212, 213, 214) 'Checkout boundaries and installed app selection'

# Windows can report a stale parent PID reused by a newly started process.
$script:processFixture = @(
    (New-ProcessFixture 300 0 bun.exe 'bun unrelated.js' 20),
    (New-ProcessFixture 301 300 node.exe 'node Q:\AIVORelay\node_modules\vite.js' 10),
    (New-ProcessFixture 302 301 node.exe 'node unrelated.js' 5)
)
Assert-Equal (Get-SelectedIds) @(301) 'PID reuse must not expand into unrelated processes'

$script:processFixture = @(
    (New-ProcessFixture $PID 400 pwsh.exe '' 20),
    (New-ProcessFixture 400 401 node.exe 'node Q:\AIVORelay\tools\terminal.js' 10),
    (New-ProcessFixture 401 0 bun.exe 'bun terminal' 5),
    (New-ProcessFixture 402 400 node.exe 'node unrelated.js' 21),
    (New-ProcessFixture 403 $PID node.exe 'node Q:\AIVORelay\node_modules\vite.js' 21)
)
Assert-Equal (Get-SelectedIds) @(403) 'Current launcher and console ancestors are excluded'

$script:processFixture = @()
Assert-Equal (Get-SelectedIds) @() 'No processes running'
Confirm-AivoRelayLaunch -TargetRoot 'Q:\AIVORelay' -Ports @()
$script:checks++

# Default switching mode skips the old lookup even with an existing session.
$script:processFixture = @((New-ProcessFixture 500 0 aivorelay.exe ''))
$script:forbidLookup = $true
try {
    Confirm-AivoRelayLaunch -TargetRoot 'Q:\AIVORelay' -Ports @()
    $script:checks++
} finally {
    $script:forbidLookup = $false
}

# -NoSwitch refuses an existing app or dev tree without prompting/stopping it.
foreach ($fixture in @(
    (New-ProcessFixture 501 0 aivorelay.exe ''),
    (New-ProcessFixture 502 0 node.exe 'node Q:\AIVORelay\node_modules\vite.js')
)) {
    $script:processFixture = @($fixture)
    $blocked = $false
    try { Confirm-AivoRelayLaunch -TargetRoot 'Q:\AIVORelay' -Ports @() -NoSwitch }
    catch {
        if ($_.Exception.Message -notlike 'AivoRelay launch cancelled (-NoSwitch)*') { throw }
        $blocked = $true
    }
    Assert-Equal $blocked $true 'NoSwitch refuses an existing session without confirmation'
    Assert-Equal @($script:processFixture.ProcessId) @($fixture.ProcessId) 'Preflight leaves the existing session untouched'
}
$script:processFixture = @()
Confirm-AivoRelayLaunch -TargetRoot 'Q:\AIVORelay' -Ports @() -NoSwitch
$script:checks++

# Test port ownership using only an ephemeral listener created by this test.
$listener = [Net.Sockets.TcpListener]::new([Net.IPAddress]::Loopback, 0)
try {
    $listener.Start()
    $port = ([Net.IPEndPoint]$listener.LocalEndpoint).Port
    $blocked = $false
    try { Assert-AivoRelayLaunchPorts -Ports @($port) }
    catch {
        if ($_.Exception.Message -notlike "Dev port $port is still in use*") { throw }
        $blocked = $true
    }
    Assert-Equal $blocked $true 'Unknown port owners are refused without stopping them'
} finally {
    $listener.Stop()
}
Assert-AivoRelayLaunchPorts -Ports @($port, $port)
$script:checks++

Write-Host "PASS: $script:checks launcher checks; no real process was stopped."
