# Combined Build Requirements

- Ship one combined Windows distribution folder containing `AivoRelay.exe`, `aivorelay-avx2.exe`, and `aivorelay-cuda.exe`.
- Keep one shared application data/profile location so all variants use the same settings, logs, models, and history.
- Use one shared codebase; runtime logic must detect which executable variant is currently running.
- Detect local hardware at startup:
  - NVIDIA-capable system -> prefer CUDA variant
  - AVX2-capable CPU -> prefer AVX2 variant when CUDA is not preferred
  - otherwise stay on the standard variant
- If a better variant is available, show a prompt offering restart into that executable.
- Allow the user to dismiss, disable future prompts, or manually switch variants later.
- If autostart with Windows is enabled, switching variants must also update autostart so Windows launches the currently selected executable.
- Prefer release-build testing over dev-server testing because restart/handoff flow may not behave correctly in dev mode.
- First implementation can focus on local combined build output and runtime switching; release workflow changes are separate work.
