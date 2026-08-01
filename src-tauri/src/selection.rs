#[cfg(target_os = "windows")]
fn selected_text_from_uia_element(
    element: &windows::Win32::UI::Accessibility::IUIAutomationElement,
) -> Result<Option<String>, String> {
    use windows::Win32::UI::Accessibility::{
        IUIAutomationTextPattern, SupportedTextSelection_None, UIA_TextPatternId,
    };

    // SAFETY: The caller creates and uses every UIA interface on the same
    // initialized COM worker thread.
    unsafe {
        let pattern =
            match element.GetCurrentPatternAs::<IUIAutomationTextPattern>(UIA_TextPatternId) {
                Ok(pattern) => pattern,
                Err(_) => return Ok(None),
            };
        if pattern.SupportedTextSelection().map_err(|error| {
            format!("Windows accessibility could not inspect selection support: {error}")
        })? == SupportedTextSelection_None
        {
            return Ok(None);
        }

        let ranges = pattern.GetSelection().map_err(|error| {
            format!("Windows accessibility could not read the current selection: {error}")
        })?;
        let range_count = ranges.Length().map_err(|error| {
            format!("Windows accessibility could not inspect the current selection: {error}")
        })?;
        let mut selected_text = String::new();
        for index in 0..range_count {
            let range = ranges.GetElement(index).map_err(|error| {
                format!("Windows accessibility could not inspect selection range {index}: {error}")
            })?;
            let range_text = range.GetText(-1).map_err(|error| {
                format!("Windows accessibility could not read selection range {index}: {error}")
            })?;
            if !selected_text.is_empty() && !range_text.is_empty() {
                selected_text.push('\n');
            }
            selected_text.push_str(&range_text.to_string());
        }

        Ok((!selected_text.trim().is_empty()).then_some(selected_text))
    }
}

#[cfg(target_os = "windows")]
fn document_control_type_variant() -> windows::Win32::System::Variant::VARIANT {
    use std::mem::ManuallyDrop;
    use windows::Win32::System::Variant::{VARIANT, VARIANT_0, VARIANT_0_0, VARIANT_0_0_0, VT_I4};
    use windows::Win32::UI::Accessibility::UIA_DocumentControlTypeId;

    VARIANT {
        Anonymous: VARIANT_0 {
            Anonymous: ManuallyDrop::new(VARIANT_0_0 {
                vt: VT_I4,
                wReserved1: 0,
                wReserved2: 0,
                wReserved3: 0,
                Anonymous: VARIANT_0_0_0 {
                    lVal: UIA_DocumentControlTypeId.0,
                },
            }),
        },
    }
}

#[cfg(target_os = "windows")]
fn read_selected_text_via_uia_on_com_thread(foreground_window: isize) -> Result<String, String> {
    use windows::Win32::System::Com::{
        CoCreateInstance, CoInitializeEx, CoUninitialize, CLSCTX_INPROC_SERVER,
        COINIT_MULTITHREADED,
    };
    use windows::Win32::UI::Accessibility::{
        CUIAutomation, IUIAutomation, TreeScope_Descendants, UIA_ControlTypePropertyId,
    };

    struct ComGuard;

    impl Drop for ComGuard {
        fn drop(&mut self) {
            // SAFETY: This guard is created only after CoInitializeEx succeeds
            // on this dedicated thread, and is dropped on that same thread.
            unsafe { CoUninitialize() };
        }
    }

    // SAFETY: Every COM interface created below remains on this dedicated
    // worker thread and is released before CoUninitialize runs.
    unsafe {
        CoInitializeEx(None, COINIT_MULTITHREADED)
            .ok()
            .map_err(|error| format!("Windows accessibility initialization failed: {error}"))?;
        let _com_guard = ComGuard;

        let automation: IUIAutomation =
            CoCreateInstance(&CUIAutomation, None, CLSCTX_INPROC_SERVER)
                .map_err(|error| format!("Windows accessibility is unavailable: {error}"))?;
        let walker = automation.RawViewWalker().map_err(|error| {
            format!("Windows accessibility could not inspect the focused control: {error}")
        })?;

        // Keep the old focused-element fast path, but Raw View prevents the
        // Control View filter from hiding intermediate accessibility nodes.
        if let Ok(mut candidate) = automation.GetFocusedElement() {
            for _ in 0..64 {
                if let Ok(Some(text)) = selected_text_from_uia_element(&candidate) {
                    return Ok(text);
                }
                candidate = match walker.GetParentElement(&candidate) {
                    Ok(parent) => parent,
                    Err(_) => break,
                };
            }
        }

        // Mouse selection in Chromium does not necessarily move UIA focus to
        // the page document. Search only inside the window that was foreground
        // when the hotkey fired; Document controls are required to expose the
        // Text pattern when they expose selectable document text.
        if foreground_window != 0 {
            let hwnd =
                windows::Win32::Foundation::HWND(foreground_window as *mut core::ffi::c_void);
            if let Ok(root) = automation.ElementFromHandle(hwnd) {
                let value = document_control_type_variant();
                if let Ok(condition) =
                    automation.CreatePropertyCondition(UIA_ControlTypePropertyId, &value)
                {
                    if let Ok(documents) = root.FindAll(TreeScope_Descendants, &condition) {
                        let count = documents.Length().unwrap_or(0).clamp(0, 32);
                        for index in 0..count {
                            if let Ok(document) = documents.GetElement(index) {
                                if let Ok(Some(text)) = selected_text_from_uia_element(&document) {
                                    return Ok(text);
                                }
                            }
                        }
                    }
                }
            }
        }

        Err(
            "The focused application does not expose its text selection to Windows accessibility. Use “Copy selected text, then read” for this application.".to_string()
        )
    }
}

/// Read the selection exposed by the focused application's accessibility tree.
///
/// This path does not synthesize Copy and never reads or writes the clipboard.
pub fn read_selected_text_without_copying() -> Result<String, String> {
    #[cfg(target_os = "windows")]
    {
        use windows::Win32::UI::WindowsAndMessaging::GetForegroundWindow;

        // Capture the target before any AivoRelay UI has a chance to take
        // focus. Passing the raw value keeps the HWND out of the Send boundary.
        let foreground_window = unsafe { GetForegroundWindow().0 as isize };
        std::thread::Builder::new()
            .name("tts-uia-selection".to_string())
            .spawn(move || read_selected_text_via_uia_on_com_thread(foreground_window))
            .map_err(|error| format!("Could not start the Windows accessibility reader: {error}"))?
            .join()
            .map_err(|_| "The Windows accessibility reader stopped unexpectedly.".to_string())?
    }

    #[cfg(not(target_os = "windows"))]
    {
        Err(
            "Reading selected text without copying is currently available only on Windows."
                .to_string(),
        )
    }
}
