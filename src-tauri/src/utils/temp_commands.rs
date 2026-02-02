#[tauri::command]
#[specta::specta]
pub fn change_ai_replace_allow_no_selection_setting(
    app: AppHandle,
    allowed: bool,
) -> Result<(), String> {
    let mut settings = get_settings(&app);
    settings.ai_replace_allow_no_selection = allowed;
    write_settings(&app, settings);
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub fn change_ai_replace_no_selection_system_prompt_setting(
    app: AppHandle,
    prompt: String,
) -> Result<(), String> {
    let mut settings = get_settings(&app);
    settings.ai_replace_no_selection_system_prompt = prompt;
    write_settings(&app, settings);
    Ok(())
}
