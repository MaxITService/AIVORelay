# AivoRelay AVX2 Build

This is the **AVX2-optimized version** of AivoRelay. It is **recommended for most modern computers** (2013+).

## What is AVX2?

AVX2 (Advanced Vector Extensions 2) is a set of CPU instructions that significantly accelerate the Whisper speech recognition model. Most modern Intel and AMD processors support AVX2.

## Should I use this version?

| Your situation | Recommendation |
|----------------|----------------|
| Modern PC (Intel 4th gen+ / AMD Ryzen+) | **Use AVX2 version** (this one) |
| Older PC (pre-2013) | Use the standard version |
| Unsure | Try AVX2 first - it will crash on startup if unsupported |

## How to check AVX2 support

### Windows
Open PowerShell and run:
```powershell
(Get-CimInstance Win32_Processor).Caption
```
Then search for your processor model to verify AVX2 support.

### Quick test
Simply try running this AVX2 build. If your CPU doesn't support AVX2, the application will fail to start with an error.

## Differences from standard version

| Feature | AVX2 Version | Standard Version |
|---------|--------------|------------------|
| Whisper performance | **Faster** (vectorized) | Slower (scalar) |
| CPU compatibility | 2013+ processors | All x86_64 |
| Installer name | `aivorelay-avx2_*.msi` | `aivorelay_*.msi` |
| Window title | "AivoRelay (AVX2)" | "AivoRelay" |

## Supported Processors

### Intel (AVX2 support)
- Haswell (4th gen) and newer - 2013+
- Core i3/i5/i7/i9 4xxx and newer

### AMD (AVX2 support)
- Excavator and newer - 2015+
- Ryzen (all generations)

### Not supported (use standard version)
- Intel Sandy Bridge, Ivy Bridge (2nd-3rd gen)
- AMD Bulldozer, Piledriver, Steamroller
- Very old or low-power processors

## Performance comparison

AVX2 version typically provides:
- **2-3x faster** transcription on supported hardware
- Lower CPU usage during transcription
- Better real-time performance

## Updates

**AVX2 version requires manual updates.** Auto-update is disabled because it would switch you to the standard version.

To update:
1. Click "Update manually" in the app footer, or
2. Go to [AVX2 Releases](https://github.com/MaxITService/AIVORelay/releases?q=avx2&expanded=true)
3. Download the latest `aivorelay-avx2_*.msi` file
4. Install over the existing version

## Troubleshooting

**Application crashes immediately:**
Your CPU doesn't support AVX2. Download the standard (non-AVX2) version instead.

**Application works but seems slow:**
Make sure you're using the AVX2 version if your CPU supports it. Check the window title - it should say "AivoRelay (AVX2)".

**How to identify AVX2 version:**
- Window title shows "AivoRelay (AVX2)"
- Footer shows version with "(AVX2)" suffix, e.g., "v0.7.8 (AVX2)"
- "Update manually" link instead of auto-update
