/// Gets the frontmost/active window title for context variables.
#[cfg(target_os = "windows")]
pub fn get_frontmost_app_name() -> Option<String> {
    use std::ffi::OsString;
    use std::os::windows::ffi::OsStringExt;
    use windows::Win32::UI::WindowsAndMessaging::{
        GetForegroundWindow, GetWindowTextLengthW, GetWindowTextW,
    };

    unsafe {
        let hwnd = GetForegroundWindow();
        let length = GetWindowTextLengthW(hwnd);
        if length <= 0 {
            return None;
        }

        let mut buffer: Vec<u16> = vec![0; (length + 1) as usize];
        let copied = GetWindowTextW(hwnd, &mut buffer);
        if copied <= 0 {
            return None;
        }

        buffer.truncate(copied as usize);
        let title = OsString::from_wide(&buffer)
            .to_string_lossy()
            .trim()
            .to_string();
        if title.is_empty() {
            None
        } else {
            Some(title)
        }
    }
}

#[cfg(not(target_os = "windows"))]
pub fn get_frontmost_app_name() -> Option<String> {
    None
}
