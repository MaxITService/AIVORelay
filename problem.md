# Build Problem: CUDA Integration on Windows

## Текущий Статус (2026-01-31 19:45)
**Branch:** `cuda-integration`  
**Outcome:** ⚠️ C++ часть собрана! Ошибка в Rust биндингах.

---

## Проблема Простыми Словами

У нас "сэндвич" из зависимостей, которые не работают вместе:

```
CUDA 13.1 (новый) ──требует──> C++17 стандарт
       ↓
whisper-rs-sys 0.11.1 (старый) ──не умеет──> передать флаг C++17 в nvcc
       ↓
transcribe-rs (локальный) ──замораживает──> whisper-rs на версии 0.13.2
```

### Почему это проблема?
1. **CUDA 13.1** использует библиотеки CCCL (Thrust/CUB), которые требуют C++17
2. **whisper-rs-sys 0.11.1** собирает whisper.cpp через CMake, но не передаёт флаг `--std=c++17` в nvcc
3. Мы **не можем обновить** whisper-rs до новой версии 0.15.1, потому что `transcribe-rs` зависит от 0.13.2 (конфликт native library `whisper`)

---

## Детальный Разбор Ошибок (Архив)

### 1. CUDA C++17 Requirement (Critical)
**File:** `C:/Program Files/NVIDIA GPU Computing Toolkit/CUDA/v13.1/include/cccl\cub/util_cpp_dialect.cuh(89)`  
**Error:** `fatal error C1189: #error: CUB requires at least C++17. Define CCCL_IGNORE_DEPRECATED_CPP_DIALECT to suppress this message.`

**Analysis:**
The project is using **CUDA Toolkit v13.1**. In this version, the NVIDIA CCCL (CUDA Core Compute Libraries) strictly requires C++17. However, the `whisper-rs-sys` build script (which triggers CMake for `whisper.cpp`) is not explicitly setting the `CMAKE_CUDA_STANDARD` to `17` or higher.

### 2. Clang / Bindgen Header Discovery (Discovery Blocking)
**Error:** `./whisper.cpp/ggml/include\ggml.h:207:10: fatal error: 'stdbool.h' file not found`

**Analysis:**
When `whisper-rs-sys` calls `bindgen` to generate Rust-to-C bindings, `libclang` fails to locate standard headers like `stdbool.h`. This happens on Windows because `clang` requires specific include paths for Visual Studio's CRT (C Runtime). Even inside a VS Developer Command Prompt, `libclang` doesn't automatically see the environment variables.

**Root cause discovered:** PowerShell `$env:` syntax doesn't support dashes in variable names. `BINDGEN_EXTRA_CLANG_ARGS_x86_64-pc-windows-msvc` was never actually set. Fixed by using `[Environment]::SetEnvironmentVariable()`.

### 3. Bundled Bindings Conflict (Platform Mismatch)
**Error:** `[\"Size of _G_fpos_t\"][::std::mem::size_of::<_G_fpos_t>() - 16usize];` ... `attempt to compute 12_usize - 16_usize, which would overflow`

**Analysis:**
Because bindgen fails, `whisper-rs-sys` falls back to its **bundled bindings**. These pre-generated bindings appear to be generated for **Linux (glibc)**. They reference types like `_G_fpos_t` and `_IO_FILE` which do not exist or have different sizes on Windows (MSVC).

### 4. Dependency Conflict (NEW - 2026-01-31)
**Error:** `failed to select a version for whisper-rs-sys which could resolve this conflict`

**Analysis:**
- `aivorelay` wants `whisper-rs = "0.15.1"` → requires `whisper-rs-sys = "^0.14"`
- `transcribe-rs` (local path) wants `whisper-rs = "0.13.2"` → requires `whisper-rs-sys = "^0.11"`
- Both link to native library `whisper` — cargo doesn't allow two versions

---

## Попытки Исправления (Все Провалились)

| Что пробовали | Результат |
|--------------|-----------|
| `CMAKE_CUDA_STANDARD=17` | nvcc не получил флаг — whisper-rs-sys игнорирует |
| `CMAKE_CUDA_FLAGS="--std=c++17"` | Игнорируется build.rs |
| `CUDAFLAGS="--std=c++17"` | Не подхватывается CMake |
| `NVCC_APPEND_FLAGS="--std=c++17"` | Не подхватывается |
| `BINDGEN_EXTRA_CLANG_ARGS` с пробелами | Clang разбивает пути на мусор |
| `BINDGEN_EXTRA_CLANG_ARGS` с короткими путями (DOS 8.3) | Переменная с дефисами не устанавливалась в PowerShell |
| `[Environment]::SetEnvironmentVariable` для bindgen | stdbool.h всё ещё не найден (нужно проверить) |
| Обновление whisper-rs до 0.15.1 | Конфликт с transcribe-rs |

---

## Варианты Решения

### Вариант 1: CUDA 12.4 (Рекомендуется ⭐)
- CUDA 12.4 **не требует C++17**
- Никаких изменений в коде
- Просто меняем `CUDA_PATH` в `build-cuda.ps1`

**Команда:**
```powershell
# В build-cuda.ps1 изменить:
$env:CUDA_PATH = "C:\Program Files\NVIDIA GPU Computing Toolkit\CUDA\v12.4"
```

### Вариант 2: Обновить transcribe-rs
- Если transcribe-rs твой — обновить там whisper-rs до 0.15.1
- Затем обновить whisper-rs здесь тоже до 0.15.1
- Попробовать сборку снова (может решить C++17)

### Вариант 3: Форкнуть whisper-rs-sys
- Создать патч, который форсирует C++17 через CMakeLists.txt
- Использовать форк как git-зависимость
- Самый сложный вариант

---

## Системная Информация
- **OS:** Windows 11 (x64)
- **CUDA Toolkit:** v13.1 (текущий), v12.4 (тоже установлен)
- **Visual Studio:** 2022 Community (17.14)
- **LLVM/Clang:** 21.1.8
- **whisper-rs:** 0.13.2 (заморожен из-за transcribe-rs)
- **whisper-rs-sys:** 0.11.1

---

## Файлы

- `build-cuda.ps1` — скрипт сборки (v2: короткие пути, .NET API для переменных, диагностика)
- `CUDA.md` — документация по CUDA-сборке

---

## Итоговый Выбор Стека (2026-01-31)

*   **CUDA Toolkit:** **v12.4** (Критично: версии 13.x требуют C++17, который `whisper-rs-sys 0.11.1` не умеет передавать).
*   **Генератор:** **Ninja** (Обход сломанной интеграции VS + CUDA).
*   **LLVM:** **18.0 - 21.x** (Используем 21.1.8, но помним о рисках ABI).

---

## Текущая Стратегия и Инсайды (Обновлено 2026-01-31 20:15)

Мы совершили прорыв в понимании того, почему `bindgen` ломается на Windows 11. Ниже приведен полный технический отчет для следующего агента.

### 1. Ловушка "Тихого Отката" (Silent Fallback)
Мы обнаружили, почему ошибка `overflow 208 - 216` преследует нас даже тогда, когда мы "включили" генерацию биндингов.
*   **Механизм:** В `build.rs` библиотеки `whisper-rs-sys`, если вызов `bindgen` возвращает ошибку, скрипт **не останавливает сборку**. Он печатает предупреждение `cargo:warning=Unable to generate bindings...` и **копирует встроенный bindings.rs**, сгенерированный для **Linux (glibc)**.
*   **Техническая причина Overflow:** В Linux `_G_fpos_t` занимает 216 байт, а в Windows (MSVC) — 208 байт. Код биндингов содержит проверку `[::std::mem::size_of::<_G_fpos_t>() - 216usize]`, что на Windows превращается в `208 - 216`, вызывая `integer overflow`.
*   **Вывод:** Ошибка биндингов — это на самом деле замаскированная ошибка `libclang`, который не смог найти заголовки.

### 2. Секрет `stdbool.h` и Resource Dir
`stdbool.h` — это не часть Windows SDK, а встроенный (builtin) заголовок самого Clang.
*   **Проблема:** `libclang.dll` (который использует `bindgen`) не знает, где лежат его собственные заголовки, если ему не подсказать `resource-dir`. Это корень ошибки `stdbool.h not found`.
*   **Решение:** Нужно использовать флаг `-resource-dir`. Путь к нему можно динамически получить через команду `clang.exe --print-resource-dir`. 

### 3. Исследование "Fail-Fast"
Мы изучили возможность заставить сборку падать сразу при ошибке биндгена:
*   **Вердикт:** В текущей версии `whisper-rs-sys` нет встроенного флага для этого. 
*   **Трюк для агента:** После запуска `cargo check` / `build` нужно грепать вывод на наличие `Unable to generate bindings`. Если строка есть — сборка считается невалидной, даже если `rustc` завершился успешно.

### 4. Риски версий LLVM и ABI
*   **Текущая версия:** LLVM 21.1.8.
*   **Документация `clang-sys`:** Официальная поддержка заявлена только до **LLVM 18.0**. 
*   **Опасность:** Начиная с Clang 15.0, возможны расхождения в значениях enum (напр. `EntityKind`), если не включены соответствующие фичи в `clang-sys`. Это может привести к непредсказуемому поведению биндингов. Если сборка будет вести себя странно — это первый кандидат на проверку (откат на LLVM 18).

### 5. C++ Parsing Mode
Для `whisper.cpp` (ядро на C++) рекомендуется использовать аргументы `-x c++` и `-std=c++14`, чтобы `libclang` правильно интерпретировал расширенные конструкции в заголовках.

---

## Предлагаемый Скрипт V5 ("Sentinel Edition")

Этот блок кода объединяет лучшие практики (цитирование путей, динамический поиск resource-dir и использование переменной `INCLUDE`).

```powershell
# 1. Поиск путей Clang
$LLVM = "C:\Program Files\LLVM"
$LLVM_BIN = Join-Path $LLVM "bin"
$clangExe = Join-Path $LLVM_BIN "clang.exe"
# Прясим ресурс-директорию напрямую у компилятора
$resourceDir = & $clangExe --print-resource-dir
$clangBuiltinInclude = Join-Path $resourceDir "include"

# 2. Сбор путей Windows SDK (из окружения VsDevCmd)
$winSdkDir = $env:WindowsSdkDir
$winSdkVer = $env:WindowsSDKVersion.TrimEnd("\")
$shared = Join-Path $winSdkDir "Include\$winSdkVer\shared"
$ucrt   = Join-Path $winSdkDir "Include\$winSdkVer\ucrt"
$um     = Join-Path $winSdkDir "Include\$winSdkVer\um"
$vcInclude = Join-Path $env:VCToolsInstallDir "include"

# КРИТИЧЕСКОЕ: libclang на Windows лучше находит SDK через переменную INCLUDE
$env:INCLUDE = "$vcInclude;$ucrt;$um;$shared"
[Environment]::SetEnvironmentVariable("INCLUDE", $env:INCLUDE, "Process")

# 3. Формирование аргументов Bindgen (Shell-style quoting)
$qt = { param($p) '"' + $p + '"' }
$clangArgs = @(
  "--target=x86_64-pc-windows-msvc",
  "-resource-dir", (& $qt $resourceDir),
  "-isystem",      (& $qt $clangBuiltinInclude),
  "-fms-compatibility",
  "-fms-extensions",
  "-fms-compatibility-version=19",
  "-x c++",              # Принудительный C++ режим
  "-std=c++14"           # Стандарт для парсинга
)
$clangArgsString = ($clangArgs -join " ")

# Установка переменных для Cargo
[Environment]::SetEnvironmentVariable("BINDGEN_EXTRA_CLANG_ARGS", $clangArgsString, "Process")
[Environment]::SetEnvironmentVariable("BINDGEN_EXTRA_CLANG_ARGS_x86_64-pc-windows-msvc", $clangArgsString, "Process")
[Environment]::SetEnvironmentVariable("LIBCLANG_PATH", $LLVM_BIN, "Process")
```

---

## Подробный Журнал Сессии (2026-01-31)

### 1. Диагностика (Этап "Вскрытие")
*   **Действие:** Проверка `cargo_check_cuda.txt` и `stderr` из папки `target`.
*   **Результат:** Убедились, что `cmake` и `ninja` отработали идеально. Флаг `--std=c++17` успешно проброшен в `nvcc`. 
*   **Инсайт:** Ошибка `stdbool.h` в логах биндгена подтвердила, что `libclang` "ослеп" и не видит свои же заголовки.

### 2. Исследование Bindgen (Этап "Разбор кода")
*   **Действие:** Анализ исходников `whisper-rs-sys`.
*   **Результат:** Обнаружен механизм маскировки ошибок. Библиотека прощает биндгену неудачу и подсовывает старые биндинги от Linux.
*   **Инсайт:** Это объясняет, почему все наши предыдущие попытки в `build-cuda.ps1` (V1-V4) казались успешными до момента компиляции самого Rust.

### 3. Технический Баттл (Agent Analysis)
*   **Спор:** Использовать ли `8.3` пути (Short Paths) или правильное цитирование.
*   **Решение:** Отказались от Short Paths в пользу `shell-style quoting` (`-I"path"`). 
*   **Финальный набор флагов:** Решено использовать комбинацию `-resource-dir` (от локального агента) и `INCLUDE environment + fms-compatibility` (от интернет-агента).

---

## План действий для следующего агента

1.  **Создать `build-cuda-v5.ps1`** на основе "Sentinel Edition" шаблона.
2.  **Добавить Fail-Fast:** Внедрить в скрипт проверку вывода `cargo`: `if ($output -match "Unable to generate bindings") { throw "Bindgen failed!" }`.
3.  **Локация биндингов:** После сборки проверить `src-tauri/target/debug/build/whisper-rs-sys-*/out/bindings.rs`. Он **должен** содержать Windows-специфичные типы и не иметь проверок на 216 байт.
4.  **Если статика не поможет:** Если `stdbool.h` всё еще не виден, заменить `-isystem` на `-I` для всех путей в `clangArgs`.
