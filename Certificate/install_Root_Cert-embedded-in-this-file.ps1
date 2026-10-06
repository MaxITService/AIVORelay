<#
Install-RootCA.ps1
- Auto-elevates
- Installs the embedded root certificate into LocalMachine\Root
- Verifies thumbprint before installing
- Limits the installed certificate to Code Signing (also on re-run for existing installs)
- Applies the same limit to other copies in trusted root stores, such as a copy installed for the current user only
- Informs + "Press D to delete the installed certificate, or any other key to exit"
#>

param(
  # Set by the script itself when it relaunches as Administrator. The elevated
  # window then handles only the machine stores, and the original window
  # handles the stores of the user who started the script.
  [switch]$MachineOnly
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

$ExpectedThumbprint = "CE:21:72:0C:8D:57:D5:8C:4C:17:0E:2F:C9:10:2E:2A:9C:AF:97:F5"
$EmbeddedCertBase64 = @'
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
'@
$CodeSigningOid     = "1.3.6.1.5.5.7.3.3"
$StoreCurrentUser   = 0x10000  # CERT_SYSTEM_STORE_CURRENT_USER
$StoreLocalMachine  = 0x20000  # CERT_SYSTEM_STORE_LOCAL_MACHINE
$ExitDeleted        = 2  # exit code of the Administrator window after the user chose to delete
$exitCode           = 0

# Trusted root stores the script works with.
$MachineRootStore   = @{ Location = $StoreLocalMachine; Name = "Root"; Scope = "Local Machine"; Folder = "Trusted Root Certification Authorities"; Tool = "certlm.msc" }
$OtherMachineStores = @(
  @{ Location = $StoreLocalMachine; Name = "AuthRoot"; Scope = "Local Machine"; Folder = "Third-Party Root Certification Authorities"; Tool = "certlm.msc" }
)
$UserStores         = @(
  @{ Location = $StoreCurrentUser; Name = "Root"; Scope = "Current User"; Folder = "Trusted Root Certification Authorities"; Tool = "certmgr.msc" },
  @{ Location = $StoreCurrentUser; Name = "AuthRoot"; Scope = "Current User"; Folder = "Third-Party Root Certification Authorities"; Tool = "certmgr.msc" }
)

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

# Errors thrown inside AivoRelayCertApi reach PowerShell wrapped in
# "Exception calling ...". Show only the original message.
function Get-ErrorText {
  param([Parameter(Mandatory)][System.Management.Automation.ErrorRecord]$ErrorRecord)
  $ErrorRecord.Exception.GetBaseException().Message
}

function Initialize-CertApi {
  if ('AivoRelayCertApi' -as [type]) { return }
  Add-Type -TypeDefinition @'
using System;
using System.ComponentModel;
using System.Runtime.InteropServices;

public static class AivoRelayCertApi
{
    // States of a copy in a store.
    public const int NotPresent = 0;
    public const int AlreadyLimited = 1;
    public const int AllPurposes = 2;   // no limit: trusted for every purpose
    public const int OtherPurposes = 3; // limited, but not to the expected purposes
    // Results of EnsureLimited, in addition to NotPresent and AlreadyLimited.
    public const int Limited = 4;
    public const int InstalledAndLimited = 5;

    // CERT_STORE_PROV_SYSTEM_REGISTRY_W opens only the certificates saved in that
    // exact store, without the ones Windows merges in from elsewhere (for example,
    // machine certificates that also appear in the Current User view).
    private static readonly IntPtr CERT_STORE_PROV_SYSTEM_REGISTRY_W = new IntPtr(13);
    private const uint CERT_STORE_OPEN_EXISTING_FLAG = 0x00004000;
    private const uint CERT_STORE_READONLY_FLAG = 0x00008000;
    private const uint CERT_ENCODING = 0x00010001; // X509_ASN_ENCODING | PKCS_7_ASN_ENCODING
    private const uint CERT_FIND_SHA1_HASH = 0x00010000;
    private const uint CERT_STORE_ADD_USE_EXISTING = 2;
    private const uint CERT_ENHKEY_USAGE_PROP_ID = 9; // the "Enable only the following purposes" setting
    private const int ERROR_FILE_NOT_FOUND = 2;

    [StructLayout(LayoutKind.Sequential)]
    private struct CRYPT_DATA_BLOB
    {
        public uint cbData;
        public IntPtr pbData;
    }

    [DllImport("crypt32.dll", SetLastError = true, CharSet = CharSet.Unicode)]
    private static extern IntPtr CertOpenStore(IntPtr lpszStoreProvider, uint dwEncodingType, IntPtr hCryptProv, uint dwFlags, string pvPara);

    [DllImport("crypt32.dll", SetLastError = true)]
    private static extern bool CertCloseStore(IntPtr hCertStore, uint dwFlags);

    [DllImport("crypt32.dll", SetLastError = true)]
    private static extern IntPtr CertFindCertificateInStore(IntPtr hCertStore, uint dwCertEncodingType, uint dwFindFlags, uint dwFindType, ref CRYPT_DATA_BLOB pvFindPara, IntPtr pPrevCertContext);

    [DllImport("crypt32.dll", SetLastError = true)]
    private static extern bool CertAddEncodedCertificateToStore(IntPtr hCertStore, uint dwCertEncodingType, byte[] pbCertEncoded, uint cbCertEncoded, uint dwAddDisposition, out IntPtr ppCertContext);

    [DllImport("crypt32.dll", SetLastError = true)]
    private static extern bool CertFreeCertificateContext(IntPtr pCertContext);

    [DllImport("crypt32.dll", SetLastError = true)]
    private static extern bool CertDeleteCertificateFromStore(IntPtr pCertContext);

    [DllImport("crypt32.dll", SetLastError = true)]
    private static extern bool CertGetCertificateContextProperty(IntPtr pCertContext, uint dwPropId, byte[] pvData, ref uint pcbData);

    [DllImport("crypt32.dll", SetLastError = true)]
    private static extern bool CertSetCertificateContextProperty(IntPtr pCertContext, uint dwPropId, uint dwFlags, ref CRYPT_DATA_BLOB pvData);

    // Returns IntPtr.Zero when the store does not exist.
    private static IntPtr OpenExactStore(uint location, string storeName, bool readOnly)
    {
        uint flags = location | CERT_STORE_OPEN_EXISTING_FLAG;
        if (readOnly)
        {
            flags |= CERT_STORE_READONLY_FLAG;
        }
        IntPtr store = CertOpenStore(CERT_STORE_PROV_SYSTEM_REGISTRY_W, 0, IntPtr.Zero, flags, storeName);
        if (store == IntPtr.Zero)
        {
            int error = Marshal.GetLastWin32Error();
            if (error == ERROR_FILE_NOT_FOUND)
            {
                return IntPtr.Zero;
            }
            throw new Win32Exception(error);
        }
        return store;
    }

    private static byte[] HexToBytes(string hex)
    {
        byte[] bytes = new byte[hex.Length / 2];
        for (int i = 0; i < bytes.Length; i++)
        {
            bytes[i] = Convert.ToByte(hex.Substring(i * 2, 2), 16);
        }
        return bytes;
    }

    // Returns a certificate context that the caller must free, or IntPtr.Zero.
    private static IntPtr FindCertificate(IntPtr store, byte[] sha1)
    {
        GCHandle pinned = GCHandle.Alloc(sha1, GCHandleType.Pinned);
        try
        {
            CRYPT_DATA_BLOB hash = new CRYPT_DATA_BLOB();
            hash.cbData = (uint)sha1.Length;
            hash.pbData = pinned.AddrOfPinnedObject();
            return CertFindCertificateInStore(store, CERT_ENCODING, 0, CERT_FIND_SHA1_HASH, ref hash, IntPtr.Zero);
        }
        finally
        {
            pinned.Free();
        }
    }

    // Returns null when the certificate has no usage limit in the store.
    private static byte[] GetUsage(IntPtr cert)
    {
        uint size = 0;
        if (!CertGetCertificateContextProperty(cert, CERT_ENHKEY_USAGE_PROP_ID, null, ref size))
        {
            return null;
        }
        byte[] value = new byte[size];
        if (!CertGetCertificateContextProperty(cert, CERT_ENHKEY_USAGE_PROP_ID, value, ref size))
        {
            throw new Win32Exception(Marshal.GetLastWin32Error());
        }
        Array.Resize(ref value, (int)size);
        return value;
    }

    private static void SetUsage(IntPtr cert, byte[] usage)
    {
        IntPtr buffer = Marshal.AllocHGlobal(usage.Length);
        try
        {
            Marshal.Copy(usage, 0, buffer, usage.Length);
            CRYPT_DATA_BLOB blob = new CRYPT_DATA_BLOB();
            blob.cbData = (uint)usage.Length;
            blob.pbData = buffer;
            if (!CertSetCertificateContextProperty(cert, CERT_ENHKEY_USAGE_PROP_ID, 0, ref blob))
            {
                throw new Win32Exception(Marshal.GetLastWin32Error());
            }
        }
        finally
        {
            Marshal.FreeHGlobal(buffer);
        }
    }

    private static bool SameBytes(byte[] a, byte[] b)
    {
        if (a == null || b == null || a.Length != b.Length)
        {
            return false;
        }
        for (int i = 0; i < a.Length; i++)
        {
            if (a[i] != b[i])
            {
                return false;
            }
        }
        return true;
    }

    // Reads the store without changing it.
    private static int ReadState(uint location, string storeName, byte[] sha1, byte[] usage)
    {
        IntPtr store = OpenExactStore(location, storeName, true);
        if (store == IntPtr.Zero)
        {
            return NotPresent;
        }
        try
        {
            IntPtr cert = FindCertificate(store, sha1);
            if (cert == IntPtr.Zero)
            {
                return NotPresent;
            }
            try
            {
                byte[] current = GetUsage(cert);
                if (current == null)
                {
                    return AllPurposes;
                }
                return SameBytes(current, usage) ? AlreadyLimited : OtherPurposes;
            }
            finally
            {
                CertFreeCertificateContext(cert);
            }
        }
        finally
        {
            CertCloseStore(store, 0);
        }
    }

    public static int GetState(uint location, string storeName, string thumbprint, byte[] usage)
    {
        return ReadState(location, storeName, HexToBytes(thumbprint), usage);
    }

    // Makes sure the certificate in this exact store carries the given usage limit,
    // then re-reads the store to confirm that Windows saved it. When certToAdd is
    // not null and the store has no copy, the certificate is added first.
    public static int EnsureLimited(uint location, string storeName, string thumbprint, byte[] usage, byte[] certToAdd)
    {
        byte[] sha1 = HexToBytes(thumbprint);
        int state = ReadState(location, storeName, sha1, usage);
        if (state == AlreadyLimited)
        {
            return AlreadyLimited;
        }
        if (state == NotPresent && certToAdd == null)
        {
            return NotPresent;
        }

        int result = Limited;
        IntPtr store = OpenExactStore(location, storeName, false);
        if (store == IntPtr.Zero)
        {
            throw new InvalidOperationException("The " + storeName + " certificate store was not found.");
        }
        try
        {
            IntPtr cert = FindCertificate(store, sha1);
            if (cert == IntPtr.Zero)
            {
                if (certToAdd == null)
                {
                    return NotPresent;
                }
                if (!CertAddEncodedCertificateToStore(store, CERT_ENCODING, certToAdd, (uint)certToAdd.Length, CERT_STORE_ADD_USE_EXISTING, out cert))
                {
                    throw new Win32Exception(Marshal.GetLastWin32Error());
                }
                result = InstalledAndLimited;
            }
            try
            {
                SetUsage(cert, usage);
            }
            finally
            {
                CertFreeCertificateContext(cert);
            }
        }
        finally
        {
            CertCloseStore(store, 0);
        }

        if (ReadState(location, storeName, sha1, usage) != AlreadyLimited)
        {
            throw new InvalidOperationException("Windows did not keep the Code Signing limit.");
        }
        return result;
    }

    // Deletes the certificate from this exact store, then re-reads the store to
    // confirm that it is gone. Returns false when the store has no copy.
    public static bool Remove(uint location, string storeName, string thumbprint)
    {
        byte[] sha1 = HexToBytes(thumbprint);
        if (ReadState(location, storeName, sha1, null) == NotPresent)
        {
            return false;
        }

        IntPtr store = OpenExactStore(location, storeName, false);
        if (store == IntPtr.Zero)
        {
            return false;
        }
        try
        {
            IntPtr cert = FindCertificate(store, sha1);
            if (cert == IntPtr.Zero)
            {
                return false;
            }
            // CertDeleteCertificateFromStore always frees the context, even when it fails.
            if (!CertDeleteCertificateFromStore(cert))
            {
                throw new Win32Exception(Marshal.GetLastWin32Error());
            }
        }
        finally
        {
            CertCloseStore(store, 0);
        }

        if (ReadState(location, storeName, sha1, null) != NotPresent)
        {
            throw new InvalidOperationException("The certificate is still in the store.");
        }
        return true;
    }
}
'@
}

# DER-encoded list of allowed purposes, the same format Windows stores for
# "Enable only the following purposes" in the certificate's properties.
function Get-CodeSigningOnlyUsage {
  $oids = New-Object System.Security.Cryptography.OidCollection
  [void]$oids.Add((New-Object System.Security.Cryptography.Oid($CodeSigningOid)))
  $usage = New-Object System.Security.Cryptography.X509Certificates.X509EnhancedKeyUsageExtension($oids, $false)
  ,$usage.RawData
}

# Describes what is about to change for a copy that is not limited yet.
function Get-ChangeText {
  param([Parameter(Mandatory)][int]$State)
  if ($State -eq [AivoRelayCertApi]::AllPurposes) {
    "It is currently trusted for all purposes (code signing, websites, email and more). Changing it to Code Signing only."
  } else {
    "Its current purpose settings differ from Code Signing only. Changing them to Code Signing only."
  }
}

# Returns $true when the user presses D. ConsoleKey follows the physical key,
# so this works with any keyboard layout.
function Read-DeleteChoice {
  Write-Host ""
  Write-Host "Press D to delete the installed certificate, or any other key to exit..." -ForegroundColor DarkGray
  ([Console]::ReadKey($true)).Key -eq [ConsoleKey]::D
}

# A copy in another trusted root store does not share the limit of the
# machine-wide copy, and Windows may use either copy. The certificate import
# wizard installs for "Current User" by default, so that is the usual place.
function Limit-OtherCopies {
  param(
    [Parameter(Mandatory)][string]$Thumbprint,
    [Parameter(Mandatory)][byte[]]$Usage,
    [Parameter(Mandatory)][hashtable[]]$Stores
  )

  foreach ($store in $Stores) {
    $where = "$($store.Scope)\$($store.Folder)"
    $state = [AivoRelayCertApi]::GetState($store.Location, $store.Name, $Thumbprint, $Usage)
    if ($state -eq [AivoRelayCertApi]::NotPresent) { continue }
    if ($state -eq [AivoRelayCertApi]::AlreadyLimited) {
      Write-Host "[OK] Found another copy in $where. It is already limited to Code Signing, nothing to change." -ForegroundColor Green
      continue
    }

    Write-Host "Found another copy in $where. $(Get-ChangeText $state)" -ForegroundColor Yellow
    try {
      [void][AivoRelayCertApi]::EnsureLimited($store.Location, $store.Name, $Thumbprint, $Usage, $null)
    }
    catch {
      throw "The machine-wide copy is installed and limited, but the copy in $where could not be limited ($(Get-ErrorText $_)). To finish, open $($store.Tool), go to $($store.Folder), open Max IT Service Root CA -> Properties, select 'Enable only the following purposes', and leave only Code Signing checked."
    }
    Write-Host "[OK] Changed: the copy in $where is now limited to Code Signing." -ForegroundColor Green
  }
}

# Returns the number of deleted copies.
function Remove-Copies {
  param(
    [Parameter(Mandatory)][string]$Thumbprint,
    [Parameter(Mandatory)][hashtable[]]$Stores
  )

  $removed = 0
  foreach ($store in $Stores) {
    $where = "$($store.Scope)\$($store.Folder)"
    try {
      $found = [AivoRelayCertApi]::Remove($store.Location, $store.Name, $Thumbprint)
    }
    catch {
      throw "Could not delete the certificate from $where ($(Get-ErrorText $_)). Delete Max IT Service Root CA there manually in $($store.Tool)."
    }
    if ($found) {
      Write-Host "[OK] Deleted from $where." -ForegroundColor Green
      $removed++
    }
  }
  $removed
}

$isAdministrator = Test-IsAdministrator
$expected = Format-Thumbprint $ExpectedThumbprint

try {
  Initialize-CertApi
  $codeSigningUsage = Get-CodeSigningOnlyUsage

  if (-not $isAdministrator) {
    Write-Host "Requesting Administrator elevation..." -ForegroundColor Yellow

    $scriptPath = $PSCommandPath
    if ([string]::IsNullOrWhiteSpace($scriptPath)) {
      throw "Script path is unknown. Save as a .ps1 file and run again."
    }

    $elevated = Start-Process -FilePath "powershell.exe" -Verb RunAs -PassThru -ArgumentList @(
      "-NoProfile",
      "-ExecutionPolicy", "Bypass",
      "-File", "`"$scriptPath`"",
      "-MachineOnly"
    )
    # Read the handle right away so the exit code stays available after the window closes.
    $null = $elevated.Handle
    $elevated.WaitForExit()

    # This window runs as the user who started the script, so it handles that
    # user's own stores. The Administrator window may belong to another account.
    if ($elevated.ExitCode -eq $ExitDeleted) {
      [void](Remove-Copies -Thumbprint $expected -Stores $UserStores)
    }
    elseif ($elevated.ExitCode -ne 0) {
      throw "The Administrator window reported an error, described in that window. Nothing was changed for your user account."
    }
    else {
      Limit-OtherCopies -Thumbprint $expected -Usage $codeSigningUsage -Stores $UserStores
    }
    Write-Host "[OK] Done." -ForegroundColor Green
  }
  else {
    $cert = New-Object System.Security.Cryptography.X509Certificates.X509Certificate2(,[Convert]::FromBase64String($EmbeddedCertBase64))
    $actual = Format-Thumbprint $cert.Thumbprint

    Write-Host ""
    Write-Host "Root CA certificate:" -ForegroundColor Cyan
    Write-Host "  Source:     embedded in this script"
    Write-Host "  Subject:    $($cert.Subject)"
    Write-Host "  Issuer:     $($cert.Issuer)"
    Write-Host "  Thumbprint: $actual"
    Write-Host "  Valid:      $($cert.NotBefore)  ->  $($cert.NotAfter)"
    Write-Host ""

    if ($actual -ne $expected) {
      throw "Thumbprint mismatch!`nExpected: $expected`nActual:   $actual"
    }

    # Limit the root to Code Signing, the only purpose it is used for. Windows
    # limits its built-in roots the same way. Checked on every run, so
    # re-running also updates older installs.
    $before = [AivoRelayCertApi]::GetState($StoreLocalMachine, "Root", $actual, $codeSigningUsage)
    if ($before -ne [AivoRelayCertApi]::NotPresent) {
      Write-Host "[OK] Already installed in LocalMachine\Root." -ForegroundColor Green
      if ($before -ne [AivoRelayCertApi]::AlreadyLimited) {
        Write-Host (Get-ChangeText $before) -ForegroundColor Yellow
      }
    }

    try {
      $result = [AivoRelayCertApi]::EnsureLimited($StoreLocalMachine, "Root", $actual, $codeSigningUsage, $cert.RawData)
    }
    catch {
      throw "Could not finish setting up the certificate in LocalMachine\Root ($(Get-ErrorText $_)). If it is installed, limit it manually: open certlm.msc, go to Trusted Root Certification Authorities, open Max IT Service Root CA -> Properties, select 'Enable only the following purposes', and leave only Code Signing checked."
    }

    if ($result -eq [AivoRelayCertApi]::InstalledAndLimited) {
      Write-Host "[OK] Installed into LocalMachine\Root (Trusted Root Certification Authorities)." -ForegroundColor Green
    }
    if ($result -eq [AivoRelayCertApi]::AlreadyLimited) {
      Write-Host "[OK] Limited to Code Signing: already set, nothing to change." -ForegroundColor Green
    } else {
      Write-Host "[OK] Limited to Code Signing, the only purpose AivoRelay uses it for." -ForegroundColor Green
    }

    $otherStores = $OtherMachineStores
    if (-not $MachineOnly) { $otherStores += $UserStores }
    Limit-OtherCopies -Thumbprint $actual -Usage $codeSigningUsage -Stores $otherStores
  }
}
catch {
  $exitCode = 1
  Write-Host "[ERROR] $(Get-ErrorText $_)" -ForegroundColor Red
}

if ($isAdministrator -and (Read-DeleteChoice)) {
  try {
    $deleteStores = @($MachineRootStore) + $OtherMachineStores
    if (-not $MachineOnly) { $deleteStores += $UserStores }
    $removed = Remove-Copies -Thumbprint $expected -Stores $deleteStores
    if ($removed -eq 0 -and $MachineOnly) {
      Write-Host "No machine-wide copy to delete." -ForegroundColor Yellow
    } elseif ($removed -eq 0) {
      Write-Host "Nothing to delete: the certificate is not installed." -ForegroundColor Yellow
    } else {
      Write-Host "Run this script again to reinstall the certificate." -ForegroundColor DarkGray
    }
    $exitCode = $ExitDeleted
  }
  catch {
    $exitCode = 1
    Write-Host "[ERROR] $(Get-ErrorText $_)" -ForegroundColor Red
  }
  Wait-AnyKey
}
elseif (-not $isAdministrator) {
  Wait-AnyKey
}

exit $exitCode
