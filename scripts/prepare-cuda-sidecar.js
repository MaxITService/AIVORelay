import { execFileSync } from 'child_process';
import fs from 'fs';
import path from 'path';
import { pathToFileURL } from 'url';

function chooseShortTargetDir() {
    if (process.env.AIVORELAY_CUDA_TARGET_DIR) {
        return process.env.AIVORELAY_CUDA_TARGET_DIR;
    }

    const candidates = process.platform === 'win32'
        ? ['C:\\cu', 'D:\\cu', 'C:\\t\\cu']
        : [path.join(process.cwd(), 'src-tauri', 'target', 'cuda-sidecar')];

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

function resolveCudaPath() {
    const candidates = [
        process.env.AIVORELAY_CUDA_PATH,
        process.env.CUDA_PATH,
        'C:\\Program Files\\NVIDIA GPU Computing Toolkit\\CUDA\\v12.4',
        'C:\\Program Files\\NVIDIA GPU Computing Toolkit\\CUDA\\v12.5',
        'C:\\Program Files\\NVIDIA GPU Computing Toolkit\\CUDA\\v12.6',
    ].filter(Boolean);

    return candidates.find(candidate => fs.existsSync(candidate)) ?? null;
}

function resolveLinkerEnvVar(targetTriple) {
    return `CARGO_TARGET_${targetTriple.toUpperCase().replace(/-/g, '_')}_LINKER`;
}

export function prepareCudaSidecar({ profile = 'debug' } = {}) {
    if (process.platform !== 'win32') {
        console.log('Skipping CUDA sidecar preparation on non-Windows host.');
        return null;
    }

    const cudaPath = resolveCudaPath();
    if (!cudaPath) {
        throw new Error(
            'CUDA toolkit was not found. Set AIVORELAY_CUDA_PATH or CUDA_PATH before preparing the CUDA sidecar.',
        );
    }

    const repoRoot = process.cwd();
    const explicitTargetTriple = process.env.AIVORELAY_SIDECAR_TARGET || null;
    const targetTriple = explicitTargetTriple ?? resolveHostTargetTriple();
    const cargoTargetDir = chooseShortTargetDir();
    const binaryExtension = targetTriple.includes('windows') ? '.exe' : '';
    const cargoArgs = [
        'build',
        '--manifest-path',
        path.join('src-tauri', 'Cargo.toml'),
        '-p',
        'aivorelay-cuda-sidecar',
        '--bin',
        'aivorelay-cuda',
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
        `aivorelay-cuda-${targetTriple}${binaryExtension}`,
    );
    const linkerEnvVar = resolveLinkerEnvVar(targetTriple);
    const llvmBin = 'C:\\Program Files\\LLVM\\bin';
    const buildEnv = {
        ...process.env,
        CARGO_TARGET_DIR: cargoTargetDir,
        CUDA_PATH: cudaPath,
        CMAKE_GENERATOR: 'Ninja',
        [linkerEnvVar]: process.env[linkerEnvVar] || 'lld-link',
    };

    if (fs.existsSync(llvmBin)) {
        buildEnv.PATH = `${cudaPath}\\bin;${cudaPath}\\libnvvp;${llvmBin};${process.env.PATH}`;
    } else {
        buildEnv.PATH = `${cudaPath}\\bin;${cudaPath}\\libnvvp;${process.env.PATH}`;
    }

    delete buildEnv.WHISPER_DONT_GENERATE_BINDINGS;

    console.log(`Preparing CUDA sidecar (${profile}) for ${targetTriple}...`);
    console.log(`Using CUDA toolkit at: ${cudaPath}`);
    console.log(`Using sidecar target dir: ${cargoTargetDir}`);
    fs.mkdirSync(path.dirname(tauriSidecarPath), { recursive: true });

    if (!fs.existsSync(tauriSidecarPath)) {
        // Tauri validates externalBin paths before the main build starts.
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
        `aivorelay-cuda${binaryExtension}`,
    );

    if (!fs.existsSync(builtBinaryPath)) {
        throw new Error(`CUDA sidecar binary not found at ${builtBinaryPath}`);
    }

    fs.copyFileSync(builtBinaryPath, tauriSidecarPath);
    console.log(`Prepared CUDA sidecar at ${tauriSidecarPath}`);

    return tauriSidecarPath;
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
    const profile = process.argv.includes('--release') ? 'release' : 'debug';
    prepareCudaSidecar({ profile });
}
