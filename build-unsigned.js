import { execSync } from 'child_process';
import fs from 'fs';
import path from 'path';

function resolveCargoTargetDir() {
    if (process.env.AIVORELAY_CARGO_TARGET_DIR) {
        return process.env.AIVORELAY_CARGO_TARGET_DIR;
    }

    return process.env.CARGO_TARGET_DIR;
}

// Clean up old build artifacts to avoid confusion
const cargoTargetDir = resolveCargoTargetDir();
const bundleDir = cargoTargetDir
    ? path.join(cargoTargetDir, 'release', 'bundle')
    : path.join('src-tauri', 'target', 'release', 'bundle');
const dirsToClean = ['msi', 'nsis'];

dirsToClean.forEach(dir => {
    const fullPath = path.join(bundleDir, dir);
    if (fs.existsSync(fullPath)) {
        console.log(`Cleaning old artifacts in ${dir}...`);
        fs.rmSync(fullPath, { recursive: true, force: true });
    }
});

try {
    console.log('Starting unsigned build (no code signing)...');
    console.log('Note: This build will NOT have auto-update functionality.');

    // Use --no-sign flag to skip code signing
    // Disable updater artifacts for unsigned builds (updater will still init but won't function without signing)
    const overrideConfig = JSON.stringify({
        bundle: {
            createUpdaterArtifacts: false
        }
    });

    execSync(`bun run tauri build --no-sign --config "${overrideConfig.replace(/"/g, '\\"')}"`, {
        stdio: 'inherit',
        cwd: process.cwd()
    });

    console.log('Build completed successfully!');
} catch (error) {
    console.error('Build process failed or was cancelled.');
    process.exit(1);
}
