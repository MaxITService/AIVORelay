<#
.SYNOPSIS
    Windows code signing script with maximum debug output.
    Called by Tauri's signCommand during build.

.PARAMETER FilePath
    Path to the file to sign (passed by Tauri as %1)
#>
param(
    [Parameter(Mandatory = $true, Position = 0)]
    [string]$FilePath
)

$ErrorActionPreference = "Stop"

Write-Host ""
Write-Host "========================================" -ForegroundColor Cyan
Write-Host " WINDOWS CODE SIGNING - DEBUG MODE" -ForegroundColor Cyan
Write-Host "========================================" -ForegroundColor Cyan
Write-Host ""

# --- Step 1: Log environment ---
Write-Host "[1/6] Checking environment variables..." -ForegroundColor Yellow

Write-Host "  GITHUB_ACTIONS    = $env:GITHUB_ACTIONS"
Write-Host "  RUNNER_OS         = $env:RUNNER_OS"
Write-Host "  RUNNER_TEMP       = $env:RUNNER_TEMP"

$pfxPath = $env:PFX_PATH
$pfxPassword = $env:PFX_PASSWORD

if ([string]::IsNullOrWhiteSpace($pfxPath)) {
    Write-Host "  PFX_PATH          = (NOT SET)" -ForegroundColor Red
    throw "FATAL: PFX_PATH environment variable is not set. Did the 'Restore PFX' step run?"
}
Write-Host "  PFX_PATH          = $pfxPath" -ForegroundColor Green

if ([string]::IsNullOrWhiteSpace($pfxPassword)) {
    Write-Host "  PFX_PASSWORD      = (NOT SET)" -ForegroundColor Red
    throw "FATAL: PFX_PASSWORD environment variable is not set."
}
Write-Host "  PFX_PASSWORD      = (set, $($pfxPassword.Length) chars)" -ForegroundColor Green

# --- Step 2: Validate file to sign ---
Write-Host ""
Write-Host "[2/6] Checking file to sign..." -ForegroundColor Yellow
Write-Host "  FilePath          = $FilePath"

if ([string]::IsNullOrWhiteSpace($FilePath)) {
    throw "FATAL: FilePath argument is empty. Tauri did not pass the file path correctly."
}

if (!(Test-Path -LiteralPath $FilePath)) {
    throw "FATAL: File to sign does not exist: $FilePath"
}

$fileInfo = Get-Item -LiteralPath $FilePath
Write-Host "  File exists       = YES" -ForegroundColor Green
Write-Host "  File size         = $($fileInfo.Length) bytes"
Write-Host "  File extension    = $($fileInfo.Extension)"

# --- Step 3: Validate PFX file ---
Write-Host ""
Write-Host "[3/6] Checking PFX certificate file..." -ForegroundColor Yellow

if (!(Test-Path -LiteralPath $pfxPath)) {
    throw "FATAL: PFX file does not exist at: $pfxPath"
}

$pfxInfo = Get-Item -LiteralPath $pfxPath
Write-Host "  PFX exists        = YES" -ForegroundColor Green
Write-Host "  PFX size          = $($pfxInfo.Length) bytes"

if ($pfxInfo.Length -lt 100) {
    Write-Host "  WARNING: PFX file is suspiciously small!" -ForegroundColor Red
    throw "FATAL: PFX file is too small ($($pfxInfo.Length) bytes). Base64 decode may have failed."
}

# --- Step 4: Test PFX decryption ---
Write-Host ""
Write-Host "[4/6] Testing PFX decryption (password check)..." -ForegroundColor Yellow

try {
    $securePassword = ConvertTo-SecureString -String $pfxPassword -AsPlainText -Force
    $cert = New-Object System.Security.Cryptography.X509Certificates.X509Certificate2(
        $pfxPath,
        $securePassword,
        [System.Security.Cryptography.X509Certificates.X509KeyStorageFlags]::Exportable
    )
    
    Write-Host "  Decryption        = SUCCESS" -ForegroundColor Green
    Write-Host "  Subject           = $($cert.Subject)"
    Write-Host "  Issuer            = $($cert.Issuer)"
    Write-Host "  Thumbprint        = $($cert.Thumbprint)"
    Write-Host "  NotBefore         = $($cert.NotBefore)"
    Write-Host "  NotAfter          = $($cert.NotAfter)"
    Write-Host "  HasPrivateKey     = $($cert.HasPrivateKey)"
    
    if (-not $cert.HasPrivateKey) {
        throw "FATAL: Certificate does not contain a private key. Cannot sign without private key."
    }
    
    $now = Get-Date
    if ($now -lt $cert.NotBefore -or $now -gt $cert.NotAfter) {
        Write-Host "  WARNING: Certificate is EXPIRED or NOT YET VALID!" -ForegroundColor Red
        throw "FATAL: Certificate validity period: $($cert.NotBefore) to $($cert.NotAfter). Current time: $now"
    }
    Write-Host "  Validity          = CURRENT (not expired)" -ForegroundColor Green
    
    $cert.Dispose()
}
catch [System.Security.Cryptography.CryptographicException] {
    Write-Host "  Decryption        = FAILED" -ForegroundColor Red
    throw "FATAL: Cannot decrypt PFX file. Wrong password or corrupted file. Error: $($_.Exception.Message)"
}

# --- Step 5: Find signtool.exe ---
Write-Host ""
Write-Host "[5/6] Locating signtool.exe..." -ForegroundColor Yellow

$signtool = $null

# Try PATH first
$signtoolCmd = Get-Command signtool.exe -ErrorAction SilentlyContinue
if ($signtoolCmd) {
    $signtool = $signtoolCmd.Source
    Write-Host "  Found in PATH     = $signtool" -ForegroundColor Green
}

# Fallback: search Windows SDK
if (-not $signtool) {
    Write-Host "  Not in PATH, searching Windows SDK..."
    $sdkPaths = @(
        "C:\Program Files (x86)\Windows Kits\10\bin",
        "C:\Program Files\Windows Kits\10\bin"
    )
    
    foreach ($sdkPath in $sdkPaths) {
        if (Test-Path $sdkPath) {
            $candidates = Get-ChildItem $sdkPath -Recurse -Filter signtool.exe -ErrorAction SilentlyContinue |
            Where-Object { $_.FullName -match "x64" } |
            Sort-Object { [version]($_.FullName -replace '.*\\(\d+\.\d+\.\d+\.\d+)\\.*', '$1') } -Descending
            
            if ($candidates) {
                $signtool = $candidates[0].FullName
                Write-Host "  Found in SDK      = $signtool" -ForegroundColor Green
                break
            }
        }
    }
}

if (-not $signtool) {
    throw "FATAL: signtool.exe not found. Ensure Windows SDK is installed."
}

# --- Step 6: Sign the file ---
Write-Host ""
Write-Host "[6/6] Signing file..." -ForegroundColor Yellow
Write-Host "  Command: signtool sign /f `"$pfxPath`" /p *** /fd SHA256 /tr http://timestamp.digicert.com /td SHA256 /v `"$FilePath`""
Write-Host ""

$signArgs = @(
    "sign",
    "/f", $pfxPath,
    "/p", $pfxPassword,
    "/fd", "SHA256",
    "/tr", "http://timestamp.digicert.com",
    "/td", "SHA256",
    "/v",
    $FilePath
)

& $signtool @signArgs

if ($LASTEXITCODE -ne 0) {
    Write-Host ""
    Write-Host "SIGNING FAILED with exit code $LASTEXITCODE" -ForegroundColor Red
    throw "FATAL: signtool sign failed with exit code $LASTEXITCODE"
}

Write-Host ""
Write-Host "Signing completed, verifying signature..." -ForegroundColor Yellow

& $signtool verify /pa /v $FilePath

if ($LASTEXITCODE -ne 0) {
    Write-Host ""
    Write-Host "VERIFICATION FAILED with exit code $LASTEXITCODE" -ForegroundColor Red
    throw "FATAL: signtool verify failed. The file may not be properly signed."
}

Write-Host ""
Write-Host "========================================" -ForegroundColor Green
Write-Host " SIGNING SUCCESSFUL" -ForegroundColor Green
Write-Host "========================================" -ForegroundColor Green
Write-Host ""
