<!--
Update this file before publishing when you want branch-specific lead-in notes.
GitHub Actions prepends this Markdown above GitHub-generated release notes.
-->

[Download for Windows](https://github.com/MaxITService/AIVORelay/releases/download/v1.0.56/00_AivoRelay_1.0.56_x64_en-US.msi)

### What's new

- **Administrator autostart** is more reliable: it now also starts on battery power, is no longer stopped after a few days, and a new **Repair** button fixes a broken setup in one click. These options moved to a separate developer block at the bottom of Advanced settings, and are not available in Microsoft Store installs.
- The **first-run setup** is friendlier: you can go back from the remote speech recognition choice, and it points you to more cloud services you can set up later in Models.

---

**Notice:**
**Linux and macOS builds are experimental and untested. Use them at your own risk: system-wide failures are possible.** Linux needs glibc 2.39 or newer (Ubuntu 24.04 or later). The macOS build runs on Apple Silicon only and is not notarized, so macOS may ask you to grant Accessibility and microphone access again after an update.

**Which Windows version to choose:**
- **This GitHub release** (`00_AivoRelay_1.0.56_x64_en-US.msi`) gets new versions first and updates itself. It is signed with our own certificate, so Windows may show SmartScreen or publisher warnings; to avoid them, you can install the optional [root certificate](https://github.com/MaxITService/AIVORelay/blob/main/docs/WINDOWS-CERTIFICATE-INSTALLATION.md).
- **[Microsoft Store](https://apps.microsoft.com/detail/9ppfkfh2zn1l)**: no SmartScreen warning, and the Store updates it, but new versions arrive less often.
- **Store Edition files in this release** (`aivorelay-store_1.0.56_x64_en-US.msi`, `aivorelay-store_1.0.56_x64_portable.zip`): the Store build for installing without the Store. It does not update itself. Its data stays in this computer's standard per-user folders, including when launched from the ZIP.
