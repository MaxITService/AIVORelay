# Optional: trust AivoRelay's certificate on Windows

Installing this certificate is optional. AivoRelay can be installed and used without it, although Windows may show an **Unknown publisher** or Microsoft Defender SmartScreen warning for builds downloaded from GitHub.

AivoRelay uses its own self-signed root certificate instead of a commercially issued, publicly trusted code-signing certificate. If you install this root certificate, Windows can recognize AivoRelay packages signed under it as coming from Max IT Service. The Microsoft Store edition is trusted through the Store and does not need this certificate.

> [!CAUTION]
> A trusted root certificate can authorize any software signed under it. Install it only if you want Windows to trust AivoRelay's private signing chain. Download it only from the official [MaxITService/AIVORelay releases](https://github.com/MaxITService/AIVORelay/releases) page and verify the thumbprint below.

## Expected certificate

- Subject and issuer: `Max IT Service Root CA`
- SHA-1 thumbprint: `CE21720C8D57D58C4C170E2FC9102E2A9CAF97F5`
- Valid through: December 28, 2035

Windows may display the thumbprint with spaces. The hexadecimal characters must still match exactly.

## Option 1: use the release script

1. Open the GitHub release containing the AivoRelay build you downloaded.
2. Download `install_Root_Cert-embedded-in-this-file.ps1` from its Assets section. The certificate is embedded in the script, so the separate `.cer` file is optional for this method.
3. Open Windows Terminal or PowerShell in the download folder.
4. Run:

   ```powershell
   powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\install_Root_Cert-embedded-in-this-file.ps1
   ```

5. Approve the Administrator prompt.
6. Confirm that the script shows the expected thumbprint and reports `[OK] Installed` or `[OK] Already installed`.

The script refuses to install an embedded or accompanying certificate whose thumbprint does not match the expected value.

## Option 2: install it manually

1. Download `max.root.cert.cer` from the same release's Assets section.
2. Double-click the file and select **Install Certificate**.
3. Select **Local Machine**, then approve the Administrator prompt.
4. Select **Place all certificates in the following store**.
5. Choose **Trusted Root Certification Authorities**.
6. Finish the wizard and accept the Windows security warning only after confirming the thumbprint above.

## Verify the installation

1. Press `Win+R`, enter `certlm.msc`, and press Enter.
2. Open **Trusted Root Certification Authorities → Certificates**.
3. Find **Max IT Service Root CA**.
4. Open it, select **Details → Thumbprint**, and confirm that it matches `CE21720C8D57D58C4C170E2FC9102E2A9CAF97F5`.

You can now run the AivoRelay installer downloaded from the release.

## Remove the certificate

If you no longer use GitHub-distributed AivoRelay builds:

1. Open `certlm.msc` as described above.
2. Go to **Trusted Root Certification Authorities → Certificates**.
3. Verify the certificate name and thumbprint, then delete **Max IT Service Root CA**.

Removing it may cause previously downloaded AivoRelay installers or applications signed with this certificate to appear untrusted.
