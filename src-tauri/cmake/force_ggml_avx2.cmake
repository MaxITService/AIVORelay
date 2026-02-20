# Microsoft Store Edition compatibility:
# Force whisper.cpp/ggml CPU flags to AVX2 and explicitly disable AVX512.
# This keeps one distributed binary compatible with non-AVX512 CPUs.

if(CMAKE_SOURCE_DIR MATCHES "whisper\\.cpp")
  set(GGML_NATIVE OFF CACHE BOOL "Disable host-native SIMD auto-detection" FORCE)
  set(GGML_AVX ON CACHE BOOL "Enable AVX" FORCE)
  set(GGML_AVX2 ON CACHE BOOL "Enable AVX2" FORCE)
  set(GGML_FMA ON CACHE BOOL "Enable FMA" FORCE)

  set(GGML_AVX512 OFF CACHE BOOL "Disable AVX512" FORCE)
  set(GGML_AVX512_VBMI OFF CACHE BOOL "Disable AVX512 VBMI" FORCE)
  set(GGML_AVX512_VNNI OFF CACHE BOOL "Disable AVX512 VNNI" FORCE)
  set(GGML_AVX512_BF16 OFF CACHE BOOL "Disable AVX512 BF16" FORCE)
  set(GGML_AMX_TILE OFF CACHE BOOL "Disable AMX TILE" FORCE)
  set(GGML_AMX_INT8 OFF CACHE BOOL "Disable AMX INT8" FORCE)
  set(GGML_AMX_BF16 OFF CACHE BOOL "Disable AMX BF16" FORCE)

  message(STATUS "AivoRelay Microsoft Store Edition: forcing GGML AVX2 and disabling AVX512")
endif()
