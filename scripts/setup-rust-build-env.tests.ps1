$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
. "$PSScriptRoot\setup-rust-build-env.ps1"

$script:processFixture = @()
$script:checks = 0
function Get-CimInstance {
    param([string]$ClassName, [string]$Filter, [string]$ErrorAction)
    if ($Filter) { throw 'The conflict check must use a single process snapshot.' }
    return $script:processFixture
}
function Test-Command { param([string]$Command) return $true }
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
        CreationDate = [datetime]'2026-10-16T00:00:00Z' + [timespan]::FromSeconds($Created)
    }
}
function Get-ConflictIds {
    @(Get-CargoTargetDirUsers -CargoTargetDir 'Q:\t\c' | ForEach-Object { $_.ProcessId })
}

# Other Bun/Cargo work, including a dev build in its own target dir, is not a conflict.
$script:processFixture = @(
    (New-ProcessFixture 100 0 bun.exe 'bun C:\DesktopApp\orchestrator.js'),
    (New-ProcessFixture 101 0 bun.exe 'bun test src\lib\utils.test.ts'),
    (New-ProcessFixture 110 0 bun.exe 'bun x tauri dev'),
    (New-ProcessFixture 111 110 cargo.exe 'cargo run --no-default-features' 1),
    (New-ProcessFixture 112 111 rustc.exe 'rustc --out-dir Q:\t\d\debug\deps -L dependency=Q:\t\d\debug\deps' 2),
    (New-ProcessFixture 113 111 aivorelay.exe '' 2 'Q:\t\d\debug\aivorelay.exe'),
    (New-ProcessFixture 120 0 cargo.exe 'cargo check --target-dir Q:\t\cache'),
    (New-ProcessFixture 121 0 MSBuild.exe '/nodemode:1 /nodeReuse:true'),
    (New-ProcessFixture 122 0 notepad.exe 'notepad Q:\t\c\debug\build.log'),
    (New-ProcessFixture 123 0 pwsh.exe 'pwsh -Command dir Q:\t\c')
)
Assert-Equal (Get-ConflictIds) @() 'Unrelated processes do not block tests'

# A concurrent build is reported once, by the command that owns its compilers and test binaries.
$script:processFixture = @(
    (New-ProcessFixture 200 0 pwsh.exe ''),
    (New-ProcessFixture 201 200 cargo.exe 'cargo test --lib' 1),
    (New-ProcessFixture 202 201 rustc.exe 'rustc --out-dir Q:/T/C/debug/deps' 2),
    (New-ProcessFixture 203 202 link.exe 'link.exe @C:\Temp\rustc1\linker-arguments' 3),
    (New-ProcessFixture 204 201 aivorelay_app_lib-abc.exe '' 4 'Q:\t\c\debug\deps\aivorelay_app_lib-abc.exe'),
    (New-ProcessFixture 210 0 bun.exe 'bun x tauri dev'),
    (New-ProcessFixture 211 210 node.exe 'node Q:\AIVORelay\node_modules\@tauri-apps\cli\tauri.js dev' 1),
    (New-ProcessFixture 212 211 cargo.exe 'cargo run' 2),
    (New-ProcessFixture 213 212 rustc.exe 'rustc --out-dir "Q:\t\c\debug\deps"' 3),
    (New-ProcessFixture 220 0 cargo.exe 'cargo clippy --target-dir=Q:\t\c')
)
Assert-Equal (Get-ConflictIds) @(201, 210, 220) 'Same target dir users are reported by owner'

# Windows may reuse a stale parent PID for a newer unrelated process.
$script:processFixture = @(
    (New-ProcessFixture 300 0 bun.exe 'bun unrelated.js' 20),
    (New-ProcessFixture 301 300 rustc.exe 'rustc --out-dir Q:\t\c\debug\deps' 10)
)
Assert-Equal (Get-ConflictIds) @(301) 'PID reuse does not blame an unrelated parent'

# This script's own launchers are never treated as a competing build.
$script:processFixture = @(
    (New-ProcessFixture $PID 400 pwsh.exe '' 20),
    (New-ProcessFixture 400 401 bun.exe 'bun scripts\rust-tests.ts --target-dir Q:\t\c' 10),
    (New-ProcessFixture 401 0 bun.exe 'bun terminal' 5),
    (New-ProcessFixture 402 400 rustc.exe 'rustc --out-dir Q:\t\c\debug\deps' 21)
)
Assert-Equal (Get-ConflictIds) @(402) 'Current script and its ancestors are excluded'

$script:processFixture = @(
    (New-ProcessFixture 500 0 cargo.exe ('cargo test ' + ('x' * 300))),
    (New-ProcessFixture 501 500 rustc.exe 'rustc --out-dir Q:\t\c\debug\deps' 1)
)
$message = ''
try { Initialize-RustBuildEnvironment -PreferredCargoTargetDir 'Q:\t\c' | Out-Null }
catch { $message = $_.Exception.Message }
Assert-Equal ($message -like 'Another build is using the Cargo target dir Q:\t\c.*cargo.exe 500 cargo test x*...') $true 'Conflict blocks with the owning command'
Assert-Equal ($message.Length -lt 400) $true 'Long command lines are shortened'
Write-Host "PASS: $script:checks Rust build environment checks."
