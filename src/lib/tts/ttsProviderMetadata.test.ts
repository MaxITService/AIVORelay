import assert from "node:assert";

import {
  CARTESIA_MODEL_OPTIONS,
  ELEVENLABS_MODEL_OPTIONS,
  LOCAL_TTS_INSTALL_METADATA,
  MURF_MODEL_OPTIONS,
  OPENAI_MODEL_OPTIONS,
  SONIOX_LANGUAGE_OPTIONS,
  SONIOX_MODEL_OPTIONS,
  TTS_PROVIDER_DEFAULTS,
  TTS_PROVIDER_DOCUMENTATION,
  TTS_PROVIDER_OPTIONS,
  TTS_PROVIDER_SPEED_RANGES,
} from "./ttsProviderMetadata";

const providerIds = TTS_PROVIDER_OPTIONS.map((provider) => provider.value);
assert.equal(new Set(providerIds).size, providerIds.length);
assert.deepEqual(providerIds.sort(), Object.keys(TTS_PROVIDER_DEFAULTS).sort());

for (const provider of providerIds) {
  const defaults = TTS_PROVIDER_DEFAULTS[provider];
  const [minimum, maximum] = TTS_PROVIDER_SPEED_RANGES[provider];
  assert.ok(defaults.model, `${provider} must have a model default`);
  assert.ok(Number.isFinite(defaults.speed));
  assert.ok(defaults.speed >= minimum && defaults.speed <= maximum);

  const documentation = TTS_PROVIDER_DOCUMENTATION[provider];
  for (const url of Object.values(documentation)) {
    if (!url) continue;
    assert.match(url, /^https:\/\//);
    assert.ok(!url.includes("llms.txt"), `${provider} exposes a human URL`);
  }
}

assert.ok(SONIOX_MODEL_OPTIONS.includes(TTS_PROVIDER_DEFAULTS.soniox.model));
assert.ok(OPENAI_MODEL_OPTIONS.includes(TTS_PROVIDER_DEFAULTS.openai.model));
assert.ok(MURF_MODEL_OPTIONS.includes(TTS_PROVIDER_DEFAULTS.murf.model));
assert.ok(
  ELEVENLABS_MODEL_OPTIONS.includes(TTS_PROVIDER_DEFAULTS.elevenlabs.model),
);
assert.ok(
  CARTESIA_MODEL_OPTIONS.includes(TTS_PROVIDER_DEFAULTS.cartesia.model),
);
assert.deepEqual(TTS_PROVIDER_SPEED_RANGES.cartesia, [0.6, 1.5]);
assert.equal(TTS_PROVIDER_DEFAULTS.openai_compatible.model, "tts-1");
assert.equal(TTS_PROVIDER_DEFAULTS.deepgram.model, "aura-2-thalia-en");
assert.equal(TTS_PROVIDER_DEFAULTS.edge.voice, "en-US-AriaNeural");
assert.equal(TTS_PROVIDER_DEFAULTS.local_qwen.voice, "Ryan");
assert.equal(TTS_PROVIDER_DEFAULTS.local_kokoro.voice, "af_maple");
assert.equal(
  LOCAL_TTS_INSTALL_METADATA.qwen.estimatedInstallBytes,
  16 * 1024 ** 3,
);
assert.equal(
  LOCAL_TTS_INSTALL_METADATA.kokoro.estimatedInstallBytes,
  2 * 1024 ** 3,
);

const sonioxLanguageCodes = SONIOX_LANGUAGE_OPTIONS.map(([code]) => code);
assert.equal(new Set(sonioxLanguageCodes).size, sonioxLanguageCodes.length);
assert.ok(
  sonioxLanguageCodes.some(
    (language) => language === TTS_PROVIDER_DEFAULTS.soniox.language,
  ),
);

for (const metadata of Object.values(LOCAL_TTS_INSTALL_METADATA)) {
  assert.ok(metadata.author);
  assert.ok(metadata.estimatedInstallBytes >= 1024 ** 3);
  assert.match(metadata.sourceUrl, /^https:\/\//);
  assert.match(metadata.licenseUrl, /^https:\/\//);
  assert.ok(metadata.licenseName);
}
