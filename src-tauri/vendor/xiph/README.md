# Vendored Xiph libopus

AivoRelay builds the unmodified official Xiph libopus 1.5.2 release archive
statically. The application does not download codec code during a build and
does not require a system Opus installation or runtime Opus DLL.

- Release: https://github.com/xiph/opus/releases/tag/v1.5.2
- Archive: https://github.com/xiph/opus/releases/download/v1.5.2/opus-1.5.2.tar.gz
- SHA-256: `65c1d2f78b9f2fb20082c38cbe47c951ad5839345876e46941612ee87f9a7ce1`

`build.rs` verifies this digest before extracting or compiling the archive.
Replace both the archive and pinned digest together when intentionally updating
libopus, and update the bundled license notice if upstream changes it.
