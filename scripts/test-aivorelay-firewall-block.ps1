[CmdletBinding()]
param(
    [string[]]$ProgramPath = @(),
    [switch]$NoPause
)

$ErrorActionPreference = "Stop"

Set-StrictMode -Version Latest

$RuleGroup = "AIVORelay Network Block Test"
$RuleNamePrefix = "AIVORelay Test Block"

function Wait-AfterMenuAction {
    if (-not $NoPause) {
        Write-Host ""
    }
}

function Read-MenuChoice {
    Write-Host "Choose [B/R/Q]: " -NoNewline

    while ($true) {
        $key = [Console]::ReadKey($true)

        if ($key.Key -eq [ConsoleKey]::Enter) {
            continue
        }

        if ($key.Key -eq [ConsoleKey]::Escape) {
            Write-Host "Q"
            return "Q"
        }

        $keyChar = $key.KeyChar
        if ([char]::IsControl($keyChar)) {
            continue
        }

        $choice = ([string]$keyChar).ToUpperInvariant()
        Write-Host $choice
        return $choice
    }
}

function Test-IsAdmin {
    $identity = [Security.Principal.WindowsIdentity]::GetCurrent()
    $principal = [Security.Principal.WindowsPrincipal]::new($identity)
    return $principal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)
}

function Quote-Argument {
    param([string]$Value)

    if ($Value -notmatch '[\s"]') {
        return $Value
    }

    return '"' + ($Value -replace '"', '\"') + '"'
}

function Restart-Elevated {
    $scriptPath = if ($PSCommandPath) { $PSCommandPath } else { $MyInvocation.MyCommand.Path }
    if (-not $scriptPath) {
        throw "Cannot self-elevate because the script path is unknown."
    }

    $hostExe = (Get-Process -Id $PID).Path
    $args = @(
        "-NoProfile",
        "-ExecutionPolicy",
        "Bypass",
        "-File",
        (Quote-Argument $scriptPath)
    )

    foreach ($path in $ProgramPath) {
        $args += "-ProgramPath"
        $args += (Quote-Argument $path)
    }

    if ($NoPause) {
        $args += "-NoPause"
    }

    Write-Host "Requesting administrator rights..." -ForegroundColor Yellow
    Start-Process -FilePath $hostExe -ArgumentList ($args -join " ") -Verb RunAs
    exit
}

function Resolve-CandidatePath {
    param([string]$Path)

    try {
        $resolved = Resolve-Path -LiteralPath $Path -ErrorAction Stop
        return $resolved.ProviderPath
    } catch {
        return $null
    }
}

function Get-AivoRelayProgramPaths {
    $paths = [System.Collections.Generic.List[string]]::new()
    $knownPaths = [System.Collections.Generic.List[string]]::new()

    foreach ($path in $ProgramPath) {
        if ([string]::IsNullOrWhiteSpace($path)) {
            continue
        }
        $resolved = Resolve-CandidatePath $path
        if ($resolved) {
            $paths.Add($resolved)
        } else {
            Write-Warning "Program path not found: $path"
        }
    }

    foreach ($processName in @("AivoRelay", "aivorelay")) {
        Get-Process -Name $processName -ErrorAction SilentlyContinue |
            ForEach-Object {
                try {
                    if ($_.Path) {
                        $paths.Add($_.Path)
                    }
                } catch {
                    # Some process paths may be inaccessible; ignore them.
                }
            }
    }

    $repoRoot = Resolve-Path -LiteralPath (Join-Path $PSScriptRoot "..") | Select-Object -ExpandProperty ProviderPath
    $knownPaths.Add((Join-Path $repoRoot "src-tauri\target\release\aivorelay.exe"))
    $knownPaths.Add((Join-Path $repoRoot "src-tauri\target\debug\aivorelay.exe"))
    $knownPaths.Add((Join-Path $repoRoot "target\release\aivorelay.exe"))
    $knownPaths.Add((Join-Path $repoRoot "target\debug\aivorelay.exe"))

    if ($env:LOCALAPPDATA) {
        $knownPaths.Add((Join-Path $env:LOCALAPPDATA "Programs\AivoRelay\AivoRelay.exe"))
        $knownPaths.Add((Join-Path $env:LOCALAPPDATA "Programs\aivorelay\AivoRelay.exe"))
    }
    if ($env:ProgramFiles) {
        $knownPaths.Add((Join-Path $env:ProgramFiles "AivoRelay\AivoRelay.exe"))
    }
    if (${env:ProgramFiles(x86)}) {
        $knownPaths.Add((Join-Path ${env:ProgramFiles(x86)} "AivoRelay\AivoRelay.exe"))
    }

    foreach ($path in $knownPaths) {
        $resolved = Resolve-CandidatePath $path
        if ($resolved) {
            $paths.Add($resolved)
        }
    }

    return $paths |
        Where-Object { -not [string]::IsNullOrWhiteSpace($_) } |
        Sort-Object -Unique
}

function Get-TestRules {
    Get-NetFirewallRule -Group $RuleGroup -ErrorAction SilentlyContinue
}

function Remove-TestRules {
    param(
        [string]$WhenNoneMessage = "No $RuleGroup rules to remove.",
        [string]$WhenRemovedMessage = "Removed {0} firewall rule(s)."
    )

    $rules = @(Get-TestRules)
    if ($rules.Count -eq 0) {
        if (-not [string]::IsNullOrWhiteSpace($WhenNoneMessage)) {
            Write-Host $WhenNoneMessage -ForegroundColor DarkGray
        }
        return
    }

    $rules | Remove-NetFirewallRule -ErrorAction Stop
    if (-not [string]::IsNullOrWhiteSpace($WhenRemovedMessage)) {
        Write-Host ($WhenRemovedMessage -f $rules.Count) -ForegroundColor Green
    }
}

function Add-BlockRules {
    $paths = @(Get-AivoRelayProgramPaths)
    if ($paths.Count -eq 0) {
        Write-Host "No AivoRelay executable found." -ForegroundColor Red
        Write-Host "Run with -ProgramPath `"C:\Path\To\AivoRelay.exe`"." -ForegroundColor Yellow
        return
    }

    Write-Host "Preparing clean block rules: removing old $RuleGroup rules first, if any." -ForegroundColor DarkGray
    Remove-TestRules `
        -WhenNoneMessage "No old $RuleGroup rules found; creating fresh block rules." `
        -WhenRemovedMessage "Removed {0} old test rule(s); creating fresh block rules."

    foreach ($path in $paths) {
        $safeName = Split-Path -Leaf $path
        New-NetFirewallRule `
            -DisplayName "$RuleNamePrefix - Outbound - $safeName" `
            -Group $RuleGroup `
            -Direction Outbound `
            -Action Block `
            -Program $path `
            -Profile Any `
            -Enabled True `
            -ErrorAction Stop | Out-Null
    }

    Write-Host "Blocked outbound network for $($paths.Count) AivoRelay executable path(s)." -ForegroundColor Red
}

function Invoke-MenuAction {
    param([scriptblock]$Action)

    try {
        & $Action
    } catch {
        Write-Host ""
        Write-Host "Action failed:" -ForegroundColor Red
        Write-Host $_.Exception.Message -ForegroundColor Red
        if ($_.ScriptStackTrace) {
            Write-Host ""
            Write-Host $_.ScriptStackTrace -ForegroundColor DarkGray
        }
    }
}

function Show-Status {
    $paths = @(Get-AivoRelayProgramPaths)
    $rules = @(Get-TestRules)

    Write-Host ""
    Write-Host "AIVORelay firewall block test" -ForegroundColor Cyan
    Write-Host "Rule group: $RuleGroup"
    Write-Host "Rules active: $($rules.Count)"
    Write-Host ""
    Write-Host "Detected executable paths:"
    if ($paths.Count -eq 0) {
        Write-Host "  none" -ForegroundColor DarkGray
    } else {
        foreach ($path in $paths) {
            Write-Host "  $path"
        }
    }
    Write-Host ""
}

try {
    if (-not (Test-IsAdmin)) {
        Restart-Elevated
    }

    :menu while ($true) {
        Show-Status
        Write-Host "[B] Block AivoRelay outbound"
        Write-Host "[R] Restore network / remove test rules"
        Write-Host "[Q] Restore and quit"
        Write-Host ""

        $choice = Read-MenuChoice
        switch ($choice) {
            "B" {
                Invoke-MenuAction { Add-BlockRules }
                Wait-AfterMenuAction
            }
            "R" {
                Invoke-MenuAction { Remove-TestRules }
                Wait-AfterMenuAction
            }
            "Q" {
                Invoke-MenuAction { Remove-TestRules }
                Wait-AfterMenuAction
                break menu
            }
            default {
                Write-Host "Unknown choice: $choice" -ForegroundColor Yellow
                Wait-AfterMenuAction
            }
        }
    }
} catch {
    Write-Host ""
    Write-Host "Script failed:" -ForegroundColor Red
    Write-Host $_.Exception.Message -ForegroundColor Red
    if ($_.ScriptStackTrace) {
        Write-Host ""
        Write-Host $_.ScriptStackTrace -ForegroundColor DarkGray
    }
    Wait-AfterMenuAction
} finally {
    Write-Host "Done." -ForegroundColor Cyan
}
