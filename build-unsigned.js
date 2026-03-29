import { execSync } from 'child_process';
import fs from 'fs';
import path from 'path';
import { prepareAvx2Sidecar } from './scripts/prepare-avx2-sidecar.js';
import { prepareCudaSidecar } from './scripts/prepare-cuda-sidecar.js';

function resolveCargoTargetDir() {
    if (process.env.AIVORELAY_CARGO_TARGET_DIR) {
        return process.env.AIVORELAY_CARGO_TARGET_DIR;
    }

    return process.platform === 'win32' ? 'C:\\b' : undefined;
}

function resolveAvx2CmakeInclude() {
    return path.join(process.cwd(), 'src-tauri', 'cmake', 'force_ggml_avx2.cmake');
}

function resolveExternalBinList() {
    const bins = ['binaries/aivorelay-avx2'];

    if (process.env.AIVORELAY_BUILD_CUDA === '1') {
        bins.push('binaries/aivorelay-cuda');
    }

    return bins;
}

function shouldCleanupBuildCache() {
    if (process.env.GITHUB_ACTIONS === 'true') {
        return false;
    }

    const keepBuildCache = (process.env.AIVORELAY_KEEP_BUILD_CACHE ?? '').toLowerCase();
    return !['1', 'true', 'yes'].includes(keepBuildCache);
}

function resolveManagedTargetDir(envVarName, candidates) {
    if (process.env[envVarName]) {
        return {
            dir: process.env[envVarName],
            explicit: true,
        };
    }

    for (const candidate of candidates) {
        try {
            fs.mkdirSync(candidate, { recursive: true });
            process.env[envVarName] = candidate;
            return {
                dir: candidate,
                explicit: false,
            };
        } catch {
            // Try the next candidate.
        }
    }

    const fallback = candidates[candidates.length - 1];
    process.env[envVarName] = fallback;
    return {
        dir: fallback,
        explicit: false,
    };
}

function copyFileIfExists(sourcePath, destinationPath) {
    if (fs.existsSync(sourcePath)) {
        fs.copyFileSync(sourcePath, destinationPath);
    }
}

function copyLatestMatchingFile(patternPrefix, destinationPath) {
    const directory = path.dirname(patternPrefix);
    const basenamePrefix = path.basename(patternPrefix);

    if (!fs.existsSync(directory)) {
        return;
    }

    const candidates = fs.readdirSync(directory)
        .filter(name => name.startsWith(basenamePrefix) && name.endsWith('.exe'))
        .map(name => ({
            name,
            fullPath: path.join(directory, name),
            mtimeMs: fs.statSync(path.join(directory, name)).mtimeMs,
        }))
        .sort((a, b) => b.mtimeMs - a.mtimeMs);

    if (candidates[0]) {
        fs.copyFileSync(candidates[0].fullPath, destinationPath);
    }
}

function preserveLocalBuildArtifacts({ cargoTargetDir, includeCuda }) {
    const artifactsDir = path.join(process.cwd(), '.AGENTS', '.UNTRACKED', 'build-artifacts', 'release');
    fs.rmSync(artifactsDir, { recursive: true, force: true });
    fs.mkdirSync(artifactsDir, { recursive: true });

    copyFileIfExists(
        path.join(cargoTargetDir, 'release', 'aivorelay.exe'),
        path.join(artifactsDir, 'aivorelay.exe'),
    );
    copyLatestMatchingFile(
        path.join(process.cwd(), 'src-tauri', 'binaries', 'aivorelay-avx2-'),
        path.join(artifactsDir, 'aivorelay-avx2.exe'),
    );

    if (includeCuda) {
        copyLatestMatchingFile(
            path.join(process.cwd(), 'src-tauri', 'binaries', 'aivorelay-cuda-'),
            path.join(artifactsDir, 'aivorelay-cuda.exe'),
        );
    }

    if (fs.existsSync(bundleDir)) {
        for (const name of fs.readdirSync(bundleDir).filter(name => name.endsWith('.msi'))) {
            fs.copyFileSync(path.join(bundleDir, name), path.join(artifactsDir, name));
        }
    }

    return artifactsDir;
}

function removeManagedBuildDirectory(dirPath) {
    if (!dirPath || !fs.existsSync(dirPath)) {
        return;
    }

    const resolved = path.resolve(dirPath);
    const root = path.parse(resolved).root;
    if (resolved === root) {
        throw new Error(`Refusing to delete unsafe path: ${resolved}`);
    }

    fs.rmSync(resolved, { recursive: true, force: true });
}

// Clean up old build artifacts to avoid confusion
const cargoTargetDirExplicit = Boolean(process.env.AIVORELAY_CARGO_TARGET_DIR);
const cargoTargetDir = resolveCargoTargetDir();
const avx2TargetDirState = resolveManagedTargetDir('AIVORELAY_AVX2_TARGET_DIR', ['C:\\a2', 'D:\\a2', 'C:\\t\\a2']);
const cudaTargetDirState = process.env.AIVORELAY_BUILD_CUDA === '1'
    ? resolveManagedTargetDir('AIVORELAY_CUDA_TARGET_DIR', ['C:\\cu', 'D:\\cu', 'C:\\t\\cu'])
    : { dir: process.env.AIVORELAY_CUDA_TARGET_DIR ?? null, explicit: Boolean(process.env.AIVORELAY_CUDA_TARGET_DIR) };
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
    if (process.env.AIVORELAY_BUILD_CUDA === '1') {
        prepareCudaSidecar({ profile: 'release' });
    }

    // Use --no-sign flag to skip code signing
    // Disable updater artifacts for unsigned builds (updater will still init but won't function without signing)
    const overrideConfig = JSON.stringify({
        bundle: {
            createUpdaterArtifacts: false,
            externalBin: resolveExternalBinList(),
        }
    });

    execSync(`bun run tauri build --no-sign --config "${overrideConfig.replace(/"/g, '\\"')}"`, {
        stdio: 'inherit',
        cwd: process.cwd()
    });

    if (shouldCleanupBuildCache()) {
        const artifactsDir = preserveLocalBuildArtifacts({
            cargoTargetDir,
            includeCuda: process.env.AIVORELAY_BUILD_CUDA === '1',
        });

        if (!cargoTargetDirExplicit) {
            removeManagedBuildDirectory(cargoTargetDir);
        }
        if (!avx2TargetDirState.explicit) {
            removeManagedBuildDirectory(avx2TargetDirState.dir);
        }
        if (process.env.AIVORELAY_BUILD_CUDA === '1' && !cudaTargetDirState.explicit) {
            removeManagedBuildDirectory(cudaTargetDirState.dir);
        }

        console.log(`Local build cache cleaned; preserved artifacts in ${artifactsDir}`);
    }

    console.log('Build completed successfully!');
} catch (error) {
    console.error('Build process failed or was cancelled.');
    process.exit(1);
}
