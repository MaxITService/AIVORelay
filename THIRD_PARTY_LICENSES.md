# Third-Party Licenses

This document lists the third-party components bundled with AivoRelay and their respective licenses.

---

## Vulkan Loader (vulkan-1.dll)

**Copyright:** The Khronos Group Inc., LunarG Inc.

**License:** Apache License 2.0

**Source:** https://github.com/KhronosGroup/Vulkan-Loader

The Vulkan Loader is used to provide GPU acceleration for local speech-to-text models (Whisper). It is bundled with AivoRelay on Windows to ensure compatibility on systems that don't have the Vulkan SDK installed.

```
                                 Apache License
                           Version 2.0, January 2004
                        http://www.apache.org/licenses/

TERMS AND CONDITIONS FOR USE, REPRODUCTION, AND DISTRIBUTION

1. Definitions.

   "License" shall mean the terms and conditions for use, reproduction,
   and distribution as defined by Sections 1 through 9 of this document.

   "Licensor" shall mean the copyright owner or entity authorized by
   the copyright owner that is granting the License.

   "Legal Entity" shall mean the union of the acting entity and all
   other entities that control, are controlled by, or are under common
   control with that entity.

   "You" (or "Your") shall mean an individual or Legal Entity
   exercising permissions granted by this License.

   "Source" form shall mean the preferred form for making modifications.

   "Object" form shall mean any form resulting from mechanical
   transformation or translation of a Source form.

   "Work" shall mean the work of authorship made available under the License.

   "Derivative Works" shall mean any work that is based on the Work.

   "Contribution" shall mean any work of authorship submitted to the Licensor.

   "Contributor" shall mean Licensor and any Legal Entity on behalf of whom
   a Contribution has been received by Licensor.

2. Grant of Copyright License. Subject to the terms of this License, each
   Contributor hereby grants to You a perpetual, worldwide, non-exclusive,
   no-charge, royalty-free, irrevocable copyright license to reproduce, prepare
   Derivative Works of, publicly display, publicly perform, sublicense, and
   distribute the Work and such Derivative Works in Source or Object form.

3. Grant of Patent License. Subject to the terms of this License, each
   Contributor hereby grants to You a perpetual, worldwide, non-exclusive,
   no-charge, royalty-free, irrevocable patent license to make, have made,
   use, offer to sell, sell, import, and otherwise transfer the Work.

4. Redistribution. You may reproduce and distribute copies of the Work or
   Derivative Works thereof in any medium, provided that You meet the
   following conditions:

   (a) You must give any other recipients a copy of this License; and

   (b) You must cause any modified files to carry prominent notices
       stating that You changed the files; and

   (c) You must retain all copyright, patent, trademark, and attribution
       notices from the Source form of the Work; and

   (d) If the Work includes a "NOTICE" text file, You must include a
       readable copy of the attribution notices contained within.

5. Submission of Contributions. Unless You explicitly state otherwise,
   any Contribution submitted by You shall be under the terms of this License.

6. Trademarks. This License does not grant permission to use the trade names,
   trademarks, service marks, or product names of the Licensor.

7. Disclaimer of Warranty. The Work is provided on an "AS IS" BASIS,
   WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND.

8. Limitation of Liability. In no event shall any Contributor be liable for
   any damages arising from the use of the Work.

9. Accepting Warranty or Additional Liability. You may choose to offer
   additional warranty or liability obligations for a fee.

END OF TERMS AND CONDITIONS
```

---

## Whisper.cpp

**Copyright:** Georgi Gerganov and contributors

**License:** MIT License

**Source:** https://github.com/ggerganov/whisper.cpp

High-performance C/C++ implementation of OpenAI's Whisper automatic speech recognition model. Used for local speech-to-text processing.

---

## Other Dependencies

For a complete list of Rust and JavaScript dependencies, please refer to the `Cargo.toml` and `package.json` files in the source repository.
