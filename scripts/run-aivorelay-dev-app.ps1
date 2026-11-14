# Cargo calls this only after compilation succeeds, before the app is started.
param([string]$Executable)

$ErrorActionPreference = 'Stop'
$sessionDir = $env:AIVORELAY_DEV_SESSION_DIR
if (-not $sessionDir -or -not (Test-Path -LiteralPath $sessionDir -PathType Container)) {
    throw 'The AivoRelay dev session has ended; refusing to launch an orphan app.'
}

$requestPath = Join-Path $sessionDir "request-$PID.json"
$replyPath = Join-Path $sessionDir "ready-$PID"
$request = @{ ExecutablePath = [IO.Path]::GetFullPath($Executable) }
$temporaryPath = "$requestPath.tmp"
[IO.File]::WriteAllText($temporaryPath, ($request | ConvertTo-Json -Compress))
[IO.File]::Move($temporaryPath, $requestPath)

$deadline = [DateTime]::UtcNow.AddSeconds(30)
while (-not (Test-Path -LiteralPath $replyPath)) {
    if (-not (Test-Path -LiteralPath $sessionDir) -or [DateTime]::UtcNow -ge $deadline) {
        throw 'Timed out waiting for the production-to-dev handoff; app was not started.'
    }
    Start-Sleep -Milliseconds 50
}

$startInfo = [Diagnostics.ProcessStartInfo]::new($Executable)
$startInfo.UseShellExecute = $false
foreach ($argument in $args) { $startInfo.ArgumentList.Add($argument) }
if (-not (Test-Path -LiteralPath $sessionDir) -or (Test-Path -LiteralPath (Join-Path $sessionDir 'stopping'))) {
    throw 'The dev session is stopping; app was not started.'
}
$app = [Diagnostics.Process]::Start($startInfo)
try {
    $appRecord = @{ ProcessId = $app.Id; StartTimeTicks = $app.StartTime.ToUniversalTime().Ticks }
    $appRecordPath = Join-Path $sessionDir "app-$PID.json"
    [IO.File]::WriteAllText("$appRecordPath.tmp", ($appRecord | ConvertTo-Json -Compress))
    [IO.File]::Move("$appRecordPath.tmp", $appRecordPath)
    if (Test-Path -LiteralPath (Join-Path $sessionDir 'stopping')) {
        if (-not $app.HasExited) { $app.Kill($true) }
    } else {
        Write-Host '[AivoRelay] DEV ACTIVE: dev app started.' -ForegroundColor Green
    }
    $app.WaitForExit()
    exit $app.ExitCode
}
finally {
    if (-not $app.HasExited) { $app.Kill($true) }
    $app.Dispose()
}
