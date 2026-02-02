<#
Install-RootCA.ps1
- Auto-elevates
- Installs max.root.cert.cer from the script directory into LocalMachine\Root
- Verifies thumbprint before installing
- Informs + "Press any key to exit"
#>

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

$ExpectedThumbprint = "CE:21:72:0C:8D:57:D5:8C:4C:17:0E:2F:C9:10:2E:2A:9C:AF:97:F5"
$EmbeddedCertBase64 = @'
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
'@
$CertFileName       = "max.root.cert.cer"

function Format-Thumbprint {
  param([Parameter(Mandatory)][string]$Thumbprint)
  (($Thumbprint -replace '[^0-9A-Fa-f]', '')).ToUpperInvariant()
}

function Test-IsAdministrator {
  $id = [Security.Principal.WindowsIdentity]::GetCurrent()
  $p  = New-Object Security.Principal.WindowsPrincipal($id)
  $p.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)
}

function Wait-AnyKey {
  Write-Host ""
  Write-Host "Press any key to exit..." -ForegroundColor DarkGray
  [void][Console]::ReadKey($true)
}

try {
  if (-not (Test-IsAdministrator)) {
    Write-Host "Requesting Administrator elevation..." -ForegroundColor Yellow

    $scriptPath = $PSCommandPath
    if ([string]::IsNullOrWhiteSpace($scriptPath)) {
      throw "Script path is unknown. Save as a .ps1 file and run again."
    }

    Start-Process -FilePath "powershell.exe" -Verb RunAs -ArgumentList @(
      "-NoProfile",
      "-ExecutionPolicy", "Bypass",
      "-File", "`"$scriptPath`""
    )
    return
  }

  # Try file first, fallback to embedded Base64
  $certPath = Join-Path -Path $PSScriptRoot -ChildPath $CertFileName
  if (Test-Path -LiteralPath $certPath) {
    $cert = New-Object System.Security.Cryptography.X509Certificates.X509Certificate2($certPath)
  } else {
    $certBytes = [Convert]::FromBase64String($EmbeddedCertBase64)
    $cert = New-Object System.Security.Cryptography.X509Certificates.X509Certificate2(,$certBytes)
    $certPath = "(embedded)"
  }

  $expected = Format-Thumbprint $ExpectedThumbprint
  $actual   = Format-Thumbprint $cert.Thumbprint

  Write-Host ""
  Write-Host "Root CA certificate:" -ForegroundColor Cyan
  Write-Host "  File:       $certPath"
  Write-Host "  Subject:    $($cert.Subject)"
  Write-Host "  Issuer:     $($cert.Issuer)"
  Write-Host "  Thumbprint: $actual"
  Write-Host "  Valid:      $($cert.NotBefore)  ->  $($cert.NotAfter)"
  Write-Host ""

  if ($actual -ne $expected) {
    throw "Thumbprint mismatch!`nExpected: $expected`nActual:   $actual"
  }

  $store = New-Object System.Security.Cryptography.X509Certificates.X509Store(
    [System.Security.Cryptography.X509Certificates.StoreName]::Root,
    [System.Security.Cryptography.X509Certificates.StoreLocation]::LocalMachine
  )
  $store.Open([System.Security.Cryptography.X509Certificates.OpenFlags]::ReadWrite)

  try {
    $already = $store.Certificates | Where-Object { (Format-Thumbprint $_.Thumbprint) -eq $actual }

    if ($null -ne $already) {
      Write-Host "[OK] Already installed in LocalMachine\Root." -ForegroundColor Green
    } else {
      $store.Add($cert)
      Write-Host "[OK] Installed into LocalMachine\Root (Trusted Root Certification Authorities)." -ForegroundColor Green
    }
  }
  finally {
    $store.Close()
  }
}
catch {
  Write-Host "[ERROR] $($_.Exception.Message)" -ForegroundColor Red
}
finally {
  Wait-AnyKey
}
