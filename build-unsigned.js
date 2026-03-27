import { execSync } from 'child_process';
import fs from 'fs';
import path from 'path';
import { prepareAvx2Sidecar } from './scripts/prepare-avx2-sidecar.js';

function resolveCargoTargetDir() {
    if (process.env.AIVORELAY_CARGO_TARGET_DIR) {
        return process.env.AIVORELAY_CARGO_TARGET_DIR;
    }

    return process.platform === 'win32' ? 'C:\\b' : undefined;
}

function resolveAvx2CmakeInclude() {
    return path.join(process.cwd(), 'src-tauri', 'cmake', 'force_ggml_avx2.cmake');
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

    if (cargoTargetDir) {
        fs.mkdirSync(cargoTargetDir, { recursive: true });
        process.env.CARGO_TARGET_DIR = cargoTargetDir;
        console.log(`Using short CARGO_TARGET_DIR: ${cargoTargetDir}`);
    }

    if (process.env.AIVORELAY_BUILD_AVX2 === '1') {
        process.env.RUSTFLAGS = '-C target-feature=+avx2';
        process.env.CMAKE_PROJECT_INCLUDE_BEFORE = resolveAvx2CmakeInclude();
        console.log('AVX2 build mode enabled');
    }

    prepareAvx2Sidecar({ profile: 'release' });

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
