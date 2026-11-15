const RELEASES_URL = "https://github.com/MaxITService/AIVORelay/releases/latest";

/**
 * Portable builds cannot replace their running executable. Link directly to
 * the signed MSI shipped by AivoRelay instead of making users search through
 * every release asset. Public releases currently ship a Windows x64 MSI; the
 * x64 portable build is also the supported binary under Windows-on-ARM.
 */
export function buildPortableInstallerUrl(
  version: string | undefined,
): string {
  if (!version) return RELEASES_URL;
  return `${RELEASES_URL}/download/AivoRelay_${version}_x64_en-US.msi`;
}

export const PORTABLE_RELEASES_URL = RELEASES_URL;
