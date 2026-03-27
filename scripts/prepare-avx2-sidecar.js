import { execFileSync } from 'child_process';
import fs from 'fs';
import path from 'path';
import { pathToFileURL } from 'url';

function chooseShortTargetDir() {
    if (process.env.AIVORELAY_AVX2_TARGET_DIR) {
        return process.env.AIVORELAY_AVX2_TARGET_DIR;
    }

    const candidates = process.platform === 'win32'
        ? ['D:\\a2', 'C:\\a2', 'C:\\t\\a2']
        : [path.join(process.cwd(), 'src-tauri', 'target', 'avx2-sidecar')];

    for (const candidate of candidates) {
        try {
            fs.mkdirSync(candidate, { recursive: true });
            return candidate;
        } catch {
            // Try the next candidate.
        }
    }

    return candidates[candidates.length - 1];
}

function resolveHostTargetTriple() {
    const rustcVersion = execFileSync('rustc', ['-vV'], {
        cwd: process.cwd(),
        encoding: 'utf8',
        stdio: ['ignore', 'pipe', 'inherit'],
    });
    const hostMatch = rustcVersion.match(/^host:\s+(.+)$/m);

    if (!hostMatch) {
        throw new Error('Failed to determine rustc host target triple.');
    }

    return hostMatch[1].trim();
}

function mergeRustFlags(currentValue) {
    const extraFlags = '-C target-feature=+avx2';
    if (!currentValue || !currentValue.trim()) {
        return extraFlags;
    }

    if (currentValue.includes('target-feature=+avx2')) {
        return currentValue;
    }

    return `${currentValue} ${extraFlags}`;
}

export function prepareAvx2Sidecar({ profile = 'debug' } = {}) {
    if (process.platform !== 'win32') {
        console.log('Skipping AVX2 sidecar preparation on non-Windows host.');
        return null;
    }

    const repoRoot = process.cwd();
    const explicitTargetTriple = process.env.AIVORELAY_SIDECAR_TARGET || null;
    const targetTriple = explicitTargetTriple ?? resolveHostTargetTriple();
    const cargoTargetDir = chooseShortTargetDir();
    const cmakeInclude = path.join(
        repoRoot,
        'src-tauri',
        'cmake',
        'force_ggml_avx2.cmake',
    );
    const binaryExtension = targetTriple.includes('windows') ? '.exe' : '';
    const cargoArgs = [
        'build',
        '--manifest-path',
        path.join('src-tauri', 'Cargo.toml'),
        '-p',
        'aivorelay-avx2-sidecar',
        '--bin',
        'aivorelay-avx2',
    ];

    if (explicitTargetTriple) {
        cargoArgs.push('--target', explicitTargetTriple);
    }

    if (profile === 'release') {
        cargoArgs.push('--release');
    }

    const tauriSidecarPath = path.join(
        repoRoot,
        'src-tauri',
        'binaries',
        `aivorelay-avx2-${targetTriple}${binaryExtension}`,
    );

    const buildEnv = {
        ...process.env,
        CARGO_TARGET_DIR: cargoTargetDir,
        CMAKE_PROJECT_INCLUDE_BEFORE: cmakeInclude,
        RUSTFLAGS: mergeRustFlags(process.env.RUSTFLAGS),
    };

    console.log(`Preparing AVX2 sidecar (${profile}) for ${targetTriple}...`);
    console.log(`Using sidecar target dir: ${cargoTargetDir}`);
    fs.mkdirSync(path.dirname(tauriSidecarPath), { recursive: true });

    if (!fs.existsSync(tauriSidecarPath)) {
        // Tauri's build script validates externalBin paths even when compiling just this sidecar.
        // Seed a placeholder so the real sidecar can bootstrap itself.
        fs.writeFileSync(tauriSidecarPath, '');
    }

    execFileSync('cargo', cargoArgs, {
        cwd: repoRoot,
        env: buildEnv,
        stdio: 'inherit',
    });

    const builtBinaryPath = path.join(
        cargoTargetDir,
        ...(explicitTargetTriple ? [targetTriple] : []),
        profile,
        `aivorelay-avx2${binaryExtension}`,
    );

    if (!fs.existsSync(builtBinaryPath)) {
        throw new Error(`AVX2 sidecar binary not found at ${builtBinaryPath}`);
    }

    fs.copyFileSync(builtBinaryPath, tauriSidecarPath);
    console.log(`Prepared AVX2 sidecar at ${tauriSidecarPath}`);

    return tauriSidecarPath;
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
    const profile = process.argv.includes('--release') ? 'release' : 'debug';
    prepareAvx2Sidecar({ profile });
}
