[CmdletBinding()]
param(
    [string]$ExtensionRepo = "",
    [string]$OutputZip = ""
)

$ErrorActionPreference = "Stop"

$repoRoot = Split-Path -Parent $PSScriptRoot
$defaultExtensionRepo = Join-Path (Split-Path -Parent $repoRoot) "AIVORelay-relay"
$defaultOutputZip = Join-Path $repoRoot "src-tauri\resources\browser-connector\aivorelay-extension.zip"
$stageRoot = Join-Path $repoRoot ".AGENTS\.UNTRACKED\browser-connector-bundle-stage"

if ([string]::IsNullOrWhiteSpace($ExtensionRepo)) {
    $ExtensionRepo = $defaultExtensionRepo
}

if ([string]::IsNullOrWhiteSpace($OutputZip)) {
    $OutputZip = $defaultOutputZip
}

$ExtensionRepo = [System.IO.Path]::GetFullPath($ExtensionRepo)
$OutputZip = [System.IO.Path]::GetFullPath($OutputZip)

if (-not (Test-Path -LiteralPath $ExtensionRepo)) {
    throw "Extension repo not found: $ExtensionRepo"
}

$requiredFiles = @(
    "manifest.json",
    "popup.html",
    "popup.js",
    "content-script.js",
    "floating-ui.css",
    "log.js",
    "sw.js",
    "sw-config.js",
    "sw-idb.js",
    "sw-utils.js",
    "sw-network.js",
    "sw-normalize.js",
    "sw-storage.js",
    "sw-attachments.js",
    "sw-messaging.js",
    "sw-polling.js",
    "sw-init.js",
    "aivo_icon_16x16.png",
    "aivo_icon_32x32.png",
    "aivo_icon_48x48.png",
    "aivo_icon_128x128.png"
)

$requiredDirectories = @(
    "per-website-button-clicking-mechanics"
)

if (Test-Path -LiteralPath $stageRoot) {
    Remove-Item -LiteralPath $stageRoot -Recurse -Force
}

New-Item -ItemType Directory -Path $stageRoot -Force | Out-Null

foreach ($relativePath in $requiredFiles) {
    $sourcePath = Join-Path $ExtensionRepo $relativePath
    if (-not (Test-Path -LiteralPath $sourcePath -PathType Leaf)) {
        throw "Required extension file is missing: $relativePath"
    }

    $targetPath = Join-Path $stageRoot $relativePath
    $targetParent = Split-Path -Parent $targetPath
    if ($targetParent) {
        New-Item -ItemType Directory -Path $targetParent -Force | Out-Null
    }

    Copy-Item -LiteralPath $sourcePath -Destination $targetPath -Force
}

foreach ($relativePath in $requiredDirectories) {
    $sourcePath = Join-Path $ExtensionRepo $relativePath
    if (-not (Test-Path -LiteralPath $sourcePath -PathType Container)) {
        throw "Required extension directory is missing: $relativePath"
    }

    $targetPath = Join-Path $stageRoot $relativePath
    Copy-Item -LiteralPath $sourcePath -Destination $targetPath -Recurse -Force
}

$outputDir = Split-Path -Parent $OutputZip
New-Item -ItemType Directory -Path $outputDir -Force | Out-Null

if (Test-Path -LiteralPath $OutputZip) {
    Remove-Item -LiteralPath $OutputZip -Force
}

Compress-Archive -Path (Join-Path $stageRoot "*") -DestinationPath $OutputZip -CompressionLevel Optimal

$fileCount = (Get-ChildItem -LiteralPath $stageRoot -File -Recurse | Measure-Object).Count
$zipInfo = Get-Item -LiteralPath $OutputZip

Write-Host "Bundled browser connector zip rebuilt successfully."
Write-Host "Source repo: $ExtensionRepo"
Write-Host "Output zip: $OutputZip"
Write-Host "Included files: $fileCount"
Write-Host ("Zip size: {0:N0} bytes" -f $zipInfo.Length)
