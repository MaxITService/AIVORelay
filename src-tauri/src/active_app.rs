use crate::settings::{AppSettings, TranscriptionProfile};

/// Foreground application details used for prompt context and automatic profiles.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ActiveAppContext {
    pub window_title: String,
    pub process_name: String,
    pub executable_path: String,
}

impl ActiveAppContext {
    /// Short application label suitable for compact UI surfaces.
    pub fn display_name(&self) -> Option<&str> {
        if !self.process_name.is_empty() {
            Some(self.process_name.as_str())
        } else if !self.window_title.is_empty() {
            Some(self.window_title.as_str())
        } else {
            None
        }
    }
}

/// Gets the frontmost window title for existing `${current_app}` prompt variables.
/// This intentionally avoids querying process handles or disk paths for zero overhead.
#[cfg(target_os = "windows")]
fn get_hwnd_window_title(hwnd: windows::Win32::Foundation::HWND) -> Option<String> {
    use std::ffi::OsString;
    use std::os::windows::ffi::OsStringExt;
    use windows::Win32::UI::WindowsAndMessaging::GetWindowTextW;

    unsafe {
        if hwnd.0.is_null() {
            return None;
        }

        // Windows window text is capped at 32,767 characters. A fixed buffer
        // keeps the lightweight path to exactly one title-read API call.
        let mut buffer: Vec<u16> = vec![0; 32_768];
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

#[cfg(target_os = "windows")]
pub fn get_frontmost_app_name() -> Option<String> {
    use windows::Win32::UI::WindowsAndMessaging::GetForegroundWindow;
    unsafe {
        let hwnd = GetForegroundWindow();
        get_hwnd_window_title(hwnd)
    }
}

#[cfg(not(target_os = "windows"))]
pub fn get_frontmost_app_name() -> Option<String> {
    None
}

#[cfg(target_os = "windows")]
pub fn get_frontmost_app_context() -> ActiveAppContext {
    use std::ffi::OsString;
    use std::os::windows::ffi::OsStringExt;
    use std::path::Path;
    use windows::core::PWSTR;
    use windows::Win32::Foundation::CloseHandle;
    use windows::Win32::System::Threading::{
        OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_WIN32,
        PROCESS_QUERY_LIMITED_INFORMATION,
    };
    use windows::Win32::UI::WindowsAndMessaging::{GetForegroundWindow, GetWindowThreadProcessId};

    unsafe {
        let hwnd = GetForegroundWindow();
        if hwnd.0.is_null() {
            return ActiveAppContext::default();
        }

        let window_title = get_hwnd_window_title(hwnd).unwrap_or_default();
        let mut process_id = 0u32;
        GetWindowThreadProcessId(hwnd, Some(&mut process_id));
        let executable_path = if process_id == 0 {
            String::new()
        } else {
            match OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, process_id) {
                Ok(process) => {
                    let mut buffer = vec![0u16; 32_768];
                    let mut length = buffer.len() as u32;
                    let result = QueryFullProcessImageNameW(
                        process,
                        PROCESS_NAME_WIN32,
                        PWSTR(buffer.as_mut_ptr()),
                        &mut length,
                    );
                    let _ = CloseHandle(process);
                    if result.is_ok() && length > 0 {
                        buffer.truncate(length as usize);
                        OsString::from_wide(&buffer)
                            .to_string_lossy()
                            .trim()
                            .to_string()
                    } else {
                        String::new()
                    }
                }
                Err(_) => String::new(),
            }
        };
        let process_name = Path::new(&executable_path)
            .file_name()
            .map(|name| name.to_string_lossy().trim().to_string())
            .unwrap_or_default();

        ActiveAppContext {
            window_title,
            process_name,
            executable_path,
        }
    }
}

#[cfg(not(target_os = "windows"))]
pub fn get_frontmost_app_context() -> ActiveAppContext {
    ActiveAppContext::default()
}

fn wildcard_match(pattern: &str, candidate: &str) -> bool {
    let pattern_chars = pattern.chars().collect::<Vec<_>>();
    let candidate_chars = candidate.chars().collect::<Vec<_>>();
    let mut pattern_index = 0usize;
    let mut candidate_index = 0usize;
    let mut star_index: Option<usize> = None;
    let mut match_index = 0usize;

    while candidate_index < candidate_chars.len() {
        if pattern_index < pattern_chars.len()
            && (pattern_chars[pattern_index] == '?'
                || pattern_chars[pattern_index] == candidate_chars[candidate_index])
        {
            pattern_index += 1;
            candidate_index += 1;
        } else if pattern_index < pattern_chars.len() && pattern_chars[pattern_index] == '*' {
            star_index = Some(pattern_index);
            match_index = candidate_index;
            pattern_index += 1;
        } else if let Some(star) = star_index {
            pattern_index = star + 1;
            match_index += 1;
            candidate_index = match_index;
        } else {
            return false;
        }
    }

    while pattern_index < pattern_chars.len() && pattern_chars[pattern_index] == '*' {
        pattern_index += 1;
    }
    pattern_index == pattern_chars.len()
}

fn matches_candidate(pattern: &str, candidate: &str) -> bool {
    if candidate.is_empty() {
        return false;
    }
    if pattern.contains('*') || pattern.contains('?') {
        wildcard_match(pattern, candidate)
    } else {
        candidate.contains(pattern)
    }
}

fn process_name_matches(pattern: &str, process_name: &str) -> bool {
    if process_name.is_empty() {
        return false;
    }
    if pattern.contains('*') || pattern.contains('?') {
        return wildcard_match(pattern, process_name);
    }

    let pattern_without_exe = pattern.strip_suffix(".exe").unwrap_or(pattern);
    let process_without_exe = process_name.strip_suffix(".exe").unwrap_or(process_name);
    process_name == pattern || process_without_exe == pattern_without_exe
}

/// Returns a specificity score when an automatic-profile rule matches.
///
/// Supported forms are `exe:code.exe`, `title:Visual Studio Code`,
/// `path:\\Microsoft VS Code\\`, or an unprefixed value that checks the process
/// name first and then the window title. Matching is case-insensitive. `*` and
/// `?` wildcards are supported.
fn automatic_rule_match_score(rule: &str, context: &ActiveAppContext) -> Option<usize> {
    let normalized = rule.trim().to_lowercase();
    if normalized.is_empty() {
        return None;
    }

    let (field_weight, matched) = if let Some(pattern) = normalized.strip_prefix("exe:") {
        (
            300usize,
            process_name_matches(pattern.trim(), &context.process_name.to_lowercase()),
        )
    } else if let Some(pattern) = normalized.strip_prefix("title:") {
        (
            200usize,
            matches_candidate(pattern.trim(), &context.window_title.to_lowercase()),
        )
    } else if let Some(pattern) = normalized.strip_prefix("path:") {
        let normalized_pattern = pattern.trim().replace('/', "\\");
        let normalized_path = context.executable_path.to_lowercase().replace('/', "\\");
        (
            400usize,
            matches_candidate(&normalized_pattern, &normalized_path),
        )
    } else {
        let process_match = process_name_matches(&normalized, &context.process_name.to_lowercase());
        let title_match = matches_candidate(&normalized, &context.window_title.to_lowercase());
        (
            if process_match { 300 } else { 200 },
            process_match || title_match,
        )
    };

    matched.then(|| {
        let specificity = normalized
            .chars()
            .filter(|character| !matches!(character, '*' | '?' | ' '))
            .count();
        field_weight + specificity
    })
}

/// Selects the profile with the most specific matching rule. Profile order is
/// the stable tie-breaker, so older rules do not unexpectedly lose precedence.
pub fn automatic_profile_id_for_context<'a>(
    profiles: &'a [TranscriptionProfile],
    context: &ActiveAppContext,
) -> Option<&'a str> {
    let mut best: Option<(&str, usize)> = None;
    for profile in profiles {
        for rule in &profile.automatic_app_rules {
            let Some(score) = automatic_rule_match_score(rule, context) else {
                continue;
            };
            if best.is_none_or(|(_, best_score)| score > best_score) {
                best = Some((profile.id.as_str(), score));
            }
        }
    }
    best.map(|(profile_id, _)| profile_id)
}

/// Applies an already captured foreground-app match to an in-memory settings
/// snapshot. The persisted manually active profile remains the fallback for
/// unmatched apps.
pub fn apply_automatic_profile(
    settings: &mut AppSettings,
    context: &ActiveAppContext,
) -> Option<String> {
    let profile_id =
        automatic_profile_id_for_context(&settings.transcription_profiles, context)?.to_string();
    settings.active_profile_id = profile_id.clone();
    Some(profile_id)
}

#[cfg(test)]
mod tests {
    use super::{
        apply_automatic_profile, automatic_profile_id_for_context, automatic_rule_match_score,
        ActiveAppContext,
    };
    use crate::settings::{get_default_settings, TranscriptionProfile};

    fn context() -> ActiveAppContext {
        ActiveAppContext {
            window_title: "notes.md - Visual Studio Code".to_string(),
            process_name: "Code.exe".to_string(),
            executable_path: r"C:\Program Files\Microsoft VS Code\Code.exe".to_string(),
        }
    }

    fn profile(id: &str, rules: &[&str]) -> TranscriptionProfile {
        let mut profile: TranscriptionProfile = serde_json::from_value(serde_json::json!({
            "id": id,
            "name": id,
            "language": "auto",
            "translate_to_english": false
        }))
        .unwrap();
        profile.automatic_app_rules = rules.iter().map(|rule| rule.to_string()).collect();
        profile
    }

    #[test]
    fn process_rules_accept_optional_exe_suffix_and_ignore_case() {
        let context = context();
        assert!(automatic_rule_match_score("exe:CODE", &context).is_some());
        assert!(automatic_rule_match_score("code.exe", &context).is_some());
        assert!(automatic_rule_match_score("exe:chrome.exe", &context).is_none());
    }

    #[test]
    fn title_and_path_rules_support_wildcards() {
        let context = context();
        assert!(automatic_rule_match_score("title:*visual studio code", &context).is_some());
        assert!(automatic_rule_match_score(r"path:*\microsoft vs code\*", &context).is_some());
        assert!(automatic_rule_match_score("path:*/microsoft vs code/*", &context).is_some());
    }

    #[test]
    fn most_specific_matching_rule_wins() {
        let profiles = vec![
            profile("generic", &["title:code"]),
            profile("vscode", &["exe:code.exe"]),
        ];
        assert_eq!(
            automatic_profile_id_for_context(&profiles, &context()),
            Some("vscode")
        );
    }

    #[test]
    fn applying_a_profile_uses_the_supplied_snapshot() {
        let mut settings = get_default_settings();
        settings.active_profile_id = "manual".to_string();
        settings.transcription_profiles = vec![
            profile("vscode", &["exe:code.exe"]),
            profile("browser", &["exe:chrome.exe"]),
        ];

        assert_eq!(
            apply_automatic_profile(&mut settings, &context()),
            Some("vscode".to_string())
        );
        assert_eq!(settings.active_profile_id, "vscode");
    }

    #[test]
    fn unmatched_snapshot_preserves_the_manual_profile() {
        let mut settings = get_default_settings();
        settings.active_profile_id = "manual".to_string();
        settings.transcription_profiles = vec![profile("browser", &["exe:chrome.exe"])];

        assert_eq!(apply_automatic_profile(&mut settings, &context()), None);
        assert_eq!(settings.active_profile_id, "manual");
    }

    #[test]
    fn display_name_prefers_process_then_falls_back_to_window_title() {
        let full = context();
        assert_eq!(full.display_name(), Some("Code.exe"));

        let title_only = ActiveAppContext {
            window_title: "Editor".to_string(),
            ..ActiveAppContext::default()
        };
        assert_eq!(title_only.display_name(), Some("Editor"));
        assert_eq!(ActiveAppContext::default().display_name(), None);
    }
}
