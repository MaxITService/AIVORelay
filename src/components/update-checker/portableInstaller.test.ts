import assert from "node:assert";
import {
  buildPortableInstallerUrl,
  PORTABLE_RELEASES_URL,
} from "./portableInstaller";

assert.equal(
  buildPortableInstallerUrl("1.0.25"),
  "https://github.com/MaxITService/AIVORelay/releases/latest/download/AivoRelay_1.0.25_x64_en-US.msi",
);
assert.equal(buildPortableInstallerUrl(undefined), PORTABLE_RELEASES_URL);
