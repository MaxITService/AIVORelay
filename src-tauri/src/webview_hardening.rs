#[cfg(all(target_os = "windows", not(debug_assertions)))]
pub fn disable_browser_accelerator_keys(window: &tauri::WebviewWindow) {
    let label = window.label().to_string();
    if let Err(err) = window.with_webview(move |webview| unsafe {
        use webview2_com::Microsoft::Web::WebView2::Win32::ICoreWebView2Settings3;
        use windows::core::Interface;

        let result = webview
            .controller()
            .CoreWebView2()
            .and_then(|core| core.Settings())
            .and_then(|settings| settings.cast::<ICoreWebView2Settings3>())
            .and_then(|settings| settings.SetAreBrowserAcceleratorKeysEnabled(false));

        if let Err(err) = result {
            log::warn!(
                "Failed to disable WebView2 browser accelerator keys for '{}': {}",
                label,
                err
            );
        }
    }) {
        log::warn!(
            "Failed to access WebView2 instance for '{}': {}",
            window.label(),
            err
        );
    }
}

#[cfg(any(not(target_os = "windows"), debug_assertions))]
pub fn disable_browser_accelerator_keys(_window: &tauri::WebviewWindow) {}
