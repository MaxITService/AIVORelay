#[cfg(target_os = "windows")]
fn read_selected_text_via_uia_on_com_thread() -> Result<String, String> {
    use windows::Win32::System::Com::{
        CoCreateInstance, CoInitializeEx, CoUninitialize, CLSCTX_INPROC_SERVER,
        COINIT_MULTITHREADED,
    };
    use windows::Win32::UI::Accessibility::{
        CUIAutomation, IUIAutomation, IUIAutomationTextPattern, SupportedTextSelection_None,
        UIA_TextPatternId,
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
        let focused = automation.GetFocusedElement().map_err(|error| {
            format!("Windows accessibility could not find the focused text control: {error}")
        })?;
        let walker = automation.ControlViewWalker().map_err(|error| {
            format!("Windows accessibility could not inspect the focused control: {error}")
        })?;
        let mut candidate = focused;
        let mut pattern = None;

        // Rich text controls often expose TextPattern on the document ancestor
        // rather than on the exact focused child. Walk a bounded number of
        // parents so browsers and document editors work without scanning the
        // application's entire accessibility tree.
        for _ in 0..32 {
            if let Ok(candidate_pattern) =
                candidate.GetCurrentPatternAs::<IUIAutomationTextPattern>(UIA_TextPatternId)
            {
                pattern = Some(candidate_pattern);
                break;
            }
            candidate = match walker.GetParentElement(&candidate) {
                Ok(parent) => parent,
                Err(_) => break,
            };
        }

        let pattern = pattern.ok_or_else(|| {
            "The focused application does not expose its text selection to Windows accessibility. Use “Copy selected text, then read” for this application.".to_string()
        })?;
        if pattern.SupportedTextSelection().map_err(|error| {
            format!("Windows accessibility could not inspect selection support: {error}")
        })? == SupportedTextSelection_None
        {
            return Err(
                "The focused application does not expose a selectable text range to Windows accessibility. Use “Copy selected text, then read” for this application.".to_string()
            );
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

        Ok(selected_text)
    }
}

/// Read the selection exposed by the focused application's accessibility tree.
///
/// This path does not synthesize Copy and never reads or writes the clipboard.
pub fn read_selected_text_without_copying() -> Result<String, String> {
    #[cfg(target_os = "windows")]
    {
        std::thread::Builder::new()
            .name("tts-uia-selection".to_string())
            .spawn(read_selected_text_via_uia_on_com_thread)
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
