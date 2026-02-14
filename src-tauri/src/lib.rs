mod actions;
mod active_app;
#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
mod apple_intelligence;
mod audio_feedback;
pub mod audio_toolkit;
mod clipboard;
mod commands;
mod helpers;
mod input;
mod input_source;
mod language_resolver;
mod llm_client;
mod managers;
mod overlay;
mod plus_overlay_state;
#[cfg(target_os = "windows")]
mod region_capture;
mod secure_keys;
mod session_manager;
mod settings;
mod shortcut;
mod signal_handle;
mod soniox_stream_processor;
pub mod subtitle;
mod text_replacement_decapitalize;
mod transcript_context;
mod tray;
mod tray_i18n;
mod utils;
#[cfg(debug_assertions)]
use specta_typescript::{BigIntExportBehavior, Typescript};
use tauri_specta::{collect_commands, Builder};

use env_filter::Builder as EnvFilterBuilder;
use managers::audio::AudioRecordingManager;
use managers::connector::ConnectorManager;
use managers::history::HistoryManager;
use managers::key_listener::KeyListenerState;
use managers::llm_operation::LlmOperationTracker;
use managers::model::ModelManager;
use managers::remote_stt::RemoteSttManager;
use managers::soniox_realtime::SonioxRealtimeManager;
use managers::soniox_stt::SonioxSttManager;
use managers::transcription::TranscriptionManager;
#[cfg(unix)]
use signal_hook::consts::SIGUSR2;
#[cfg(unix)]
use signal_hook::iterator::Signals;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::{Arc, Mutex};
use tauri::image::Image;

use tauri::tray::TrayIconBuilder;
use tauri::Emitter;
use tauri::{AppHandle, Manager};
use tauri_plugin_autostart::{MacosLauncher, ManagerExt};
use tauri_plugin_log::{Builder as LogBuilder, RotationStrategy, Target, TargetKind};

use crate::settings::get_settings;

// Global atomic to store the file log level filter
// We use u8 to store the log::LevelFilter as a number
pub static FILE_LOG_LEVEL: AtomicU8 = AtomicU8::new(log::LevelFilter::Debug as u8);

fn level_filter_from_u8(value: u8) -> log::LevelFilter {
    match value {
        0 => log::LevelFilter::Off,
        1 => log::LevelFilter::Error,
        2 => log::LevelFilter::Warn,
        3 => log::LevelFilter::Info,
        4 => log::LevelFilter::Debug,
        5 => log::LevelFilter::Trace,
        _ => log::LevelFilter::Trace,
    }
}

fn build_console_filter() -> env_filter::Filter {
    let mut builder = EnvFilterBuilder::new();

    match std::env::var("RUST_LOG") {
        Ok(spec) if !spec.trim().is_empty() => {
            if let Err(err) = builder.try_parse(&spec) {
                log::warn!(
                    "Ignoring invalid RUST_LOG value '{}': {}. Falling back to info-level console logging",
                    spec,
                    err
                );
                builder.filter_level(log::LevelFilter::Info);
            }
        }
        _ => {
            builder.filter_level(log::LevelFilter::Info);
        }
    }

    builder.build()
}

#[derive(Default)]
struct ShortcutToggleStates {
    // Map: shortcut_binding_id -> is_active
    active_toggles: HashMap<String, bool>,
}

type ManagedToggleState = Mutex<ShortcutToggleStates>;

#[derive(Default)]
pub struct PressTimestamps {
    // Map: shortcut_binding_id -> press start time
    pub timestamps: HashMap<String, std::time::Instant>,
}

pub type ManagedPressTimestamps = Mutex<PressTimestamps>;

fn show_main_window(app: &AppHandle) {
    if let Some(main_window) = app.get_webview_window("main") {
        // First, ensure the window is visible
        if let Err(e) = main_window.show() {
            log::error!("Failed to show window: {}", e);
        }
        // Then, bring it to the front and give it focus
        if let Err(e) = main_window.set_focus() {
            log::error!("Failed to focus window: {}", e);
        }
        // Optional: On macOS, ensure the app becomes active if it was an accessory
        #[cfg(target_os = "macos")]
        {
            if let Err(e) = app.set_activation_policy(tauri::ActivationPolicy::Regular) {
                log::error!("Failed to set activation policy to Regular: {}", e);
            }
        }
    } else {
        log::error!("Main window not found.");
    }
}

fn initialize_core_logic(app_handle: &AppHandle) {
    // Initialize the input state (Enigo singleton for keyboard/mouse simulation)
    let enigo_state = input::EnigoState::new().expect("Failed to initialize input state (Enigo)");
    app_handle.manage(enigo_state);

    // Initialize the managers
    let recording_manager = Arc::new(
        AudioRecordingManager::new(app_handle).expect("Failed to initialize recording manager"),
    );
    let model_manager =
        Arc::new(ModelManager::new(app_handle).expect("Failed to initialize model manager"));
    let transcription_manager = Arc::new(
        TranscriptionManager::new(app_handle, model_manager.clone())
            .expect("Failed to initialize transcription manager"),
    );
    let remote_stt_manager =
        Arc::new(RemoteSttManager::new(app_handle).expect("Failed to initialize remote STT"));
    let soniox_realtime_manager = Arc::new(
        SonioxRealtimeManager::new(app_handle).expect("Failed to initialize Soniox realtime STT"),
    );
    let soniox_stt_manager =
        Arc::new(SonioxSttManager::new(app_handle).expect("Failed to initialize Soniox STT"));
    let history_manager =
        Arc::new(HistoryManager::new(app_handle).expect("Failed to initialize history manager"));
    let connector_manager = Arc::new(
        ConnectorManager::new(app_handle).expect("Failed to initialize connector manager"),
    );
    let llm_operation_tracker = Arc::new(LlmOperationTracker::new());

    // Initialize key listener
    let key_listener_state = KeyListenerState::new(app_handle.clone());

    // Add managers to Tauri's managed state
    app_handle.manage(recording_manager.clone());
    app_handle.manage(model_manager.clone());
    app_handle.manage(transcription_manager.clone());
    app_handle.manage(remote_stt_manager.clone());
    app_handle.manage(soniox_realtime_manager.clone());
    app_handle.manage(soniox_stt_manager.clone());
    app_handle.manage(llm_operation_tracker.clone());
    app_handle.manage(history_manager.clone());
    app_handle.manage(connector_manager.clone());
    app_handle.manage(key_listener_state);

    // Initialize region capture state (Windows only)
    #[cfg(target_os = "windows")]
    app_handle.manage(std::sync::Mutex::new(
        region_capture::RegionCaptureState::default(),
    ));

    // Start the connector server for extension communication
    if let Err(e) = connector_manager.start_server() {
        log::error!("Failed to start connector server: {}", e);
    }

    // Initialize the shortcuts
    shortcut::init_shortcuts(app_handle);

    #[cfg(unix)]
    let signals = Signals::new(&[SIGUSR2]).unwrap();
    // Set up SIGUSR2 signal handler for toggling transcription
    #[cfg(unix)]
    signal_handle::setup_signal_handler(app_handle.clone(), signals);

    // Apply macOS Accessory policy if starting hidden
    #[cfg(target_os = "macos")]
    {
        let settings = settings::get_settings(app_handle);
        if settings.start_hidden {
            let _ = app_handle.set_activation_policy(tauri::ActivationPolicy::Accessory);
        }
    }
    // Get the current theme to set the appropriate initial icon
    let initial_theme = tray::get_current_theme(app_handle);

    // Choose the appropriate initial icon based on theme
    let initial_icon_path = tray::get_icon_path(initial_theme, tray::TrayIconState::Idle);

    let tray = TrayIconBuilder::new()
        .icon(
            Image::from_path(
                app_handle
                    .path()
                    .resolve(initial_icon_path, tauri::path::BaseDirectory::Resource)
                    .unwrap(),
            )
            .unwrap(),
        )
        .show_menu_on_left_click(false)
        .icon_as_template(true)
        .on_tray_icon_event(|tray, event| {
            if let tauri::tray::TrayIconEvent::DoubleClick {
                button: tauri::tray::MouseButton::Left,
                ..
            } = event
            {
                show_main_window(tray.app_handle());
            }
        })
        .on_menu_event(|app, event| match event.id.as_ref() {
            "settings" => {
                show_main_window(app);
            }
            "check_updates" => {
                let settings = settings::get_settings(app);
                if settings.update_checks_enabled {
                    show_main_window(app);
                    let _ = app.emit("check-for-updates", ());
                }
            }
            "copy_last_transcript" => {
                tray::copy_last_transcript(app);
            }
            "unload_model" => {
                let transcription_manager = app.state::<Arc<TranscriptionManager>>();
                if !transcription_manager.is_model_loaded() {
                    log::warn!("No model is currently loaded.");
                    return;
                }
                match transcription_manager.unload_model() {
                    Ok(()) => log::info!("Model unloaded via tray."),
                    Err(e) => log::error!("Failed to unload model via tray: {}", e),
                }
            }
            "cancel" => {
                use crate::utils::cancel_current_operation;

                // Use centralized cancellation that handles all operations
                cancel_current_operation(app);
            }
            "quit" => {
                app.exit(0);
            }
            _ => {}
        })
        .build(app_handle)
        .unwrap();
    app_handle.manage(tray);

    // Initialize tray menu with idle state
    utils::update_tray_menu(app_handle, &utils::TrayIconState::Idle, None);

    // Apply tray visibility setting on startup
    let settings = settings::get_settings(app_handle);
    if !settings.show_tray_icon {
        tray::set_tray_visibility(app_handle, false);
    }

    // Get the autostart manager and configure based on user setting
    let autostart_manager = app_handle.autolaunch();
    let settings = settings::get_settings(&app_handle);

    if settings.autostart_enabled {
        // Enable autostart if user has opted in
        let _ = autostart_manager.enable();
    } else {
        // Disable autostart if user has opted out
        let _ = autostart_manager.disable();
    }

    // Create the recording overlay window (hidden by default)
    utils::create_recording_overlay(app_handle);
    // Create Soniox live preview window (hidden by default)
    utils::create_soniox_live_preview_window(app_handle);
}

#[tauri::command]
#[specta::specta]
fn trigger_update_check(app: AppHandle) -> Result<(), String> {
    let settings = settings::get_settings(&app);
    if !settings.update_checks_enabled {
        return Ok(());
    }
    app.emit("check-for-updates", ())
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Parse console logging directives from RUST_LOG, falling back to info-level logging
    // when the variable is unset
    let console_filter = build_console_filter();

    let specta_builder = Builder::<tauri::Wry>::new().commands(collect_commands![
        shortcut::change_binding,
        shortcut::reset_binding,
        shortcut::change_ptt_setting,
        shortcut::change_audio_feedback_setting,
        shortcut::change_audio_feedback_volume_setting,
        shortcut::change_sound_theme_setting,
        shortcut::change_start_hidden_setting,
        shortcut::change_autostart_setting,
        shortcut::change_translate_to_english_setting,
        shortcut::change_selected_language_setting,
        shortcut::change_transcription_provider_setting,
        shortcut::change_transcription_prompt_setting,
        shortcut::change_overlay_position_setting,
        shortcut::change_soniox_live_preview_enabled_setting,
        shortcut::change_soniox_live_preview_position_setting,
        shortcut::change_soniox_live_preview_size_setting,
        shortcut::change_soniox_live_preview_theme_setting,
        shortcut::change_soniox_live_preview_opacity_setting,
        shortcut::change_soniox_live_preview_font_color_setting,
        shortcut::change_soniox_live_preview_accent_color_setting,
        shortcut::change_soniox_live_preview_interim_opacity_setting,
        shortcut::change_debug_mode_setting,
        shortcut::change_word_correction_threshold_setting,
        shortcut::change_paste_method_setting,
        shortcut::change_paste_delay_ms_setting,
        shortcut::change_clipboard_handling_setting,
        shortcut::change_auto_submit_setting,
        shortcut::change_auto_submit_key_setting,
        shortcut::change_convert_lf_to_crlf_setting,
        shortcut::change_remote_stt_base_url_setting,
        shortcut::change_remote_stt_model_id_setting,
        shortcut::change_soniox_model_setting,
        shortcut::change_soniox_timeout_setting,
        shortcut::change_soniox_live_enabled_setting,
        shortcut::change_soniox_language_hints_setting,
        shortcut::change_soniox_context_general_json_setting,
        shortcut::change_soniox_context_text_setting,
        shortcut::change_soniox_context_terms_setting,
        shortcut::change_soniox_use_profile_language_hint_only_setting,
        shortcut::change_soniox_language_hints_strict_setting,
        shortcut::change_soniox_endpoint_detection_setting,
        shortcut::change_soniox_max_endpoint_delay_ms_setting,
        shortcut::change_soniox_language_identification_setting,
        shortcut::change_soniox_speaker_diarization_setting,
        shortcut::change_soniox_keepalive_interval_seconds_setting,
        shortcut::change_soniox_live_finalize_timeout_ms_setting,
        shortcut::change_soniox_live_instant_stop_setting,
        shortcut::change_soniox_realtime_fuzzy_correction_enabled_setting,
        shortcut::change_soniox_realtime_keep_safety_buffer_enabled_setting,
        shortcut::reset_soniox_settings_to_defaults,
        shortcut::change_remote_stt_debug_capture_setting,
        shortcut::change_remote_stt_debug_mode_setting,
        shortcut::change_post_process_enabled_setting,
        shortcut::change_post_process_reasoning_enabled_setting,
        shortcut::change_post_process_reasoning_budget_setting,
        shortcut::change_ai_replace_reasoning_enabled_setting,
        shortcut::change_ai_replace_reasoning_budget_setting,
        shortcut::change_voice_command_reasoning_enabled_setting,
        shortcut::change_voice_command_reasoning_budget_setting,
        shortcut::change_voice_command_enabled_setting,
        shortcut::change_voice_command_llm_fallback_setting,
        shortcut::change_voice_command_system_prompt_setting,
        shortcut::change_voice_command_template_setting,
        shortcut::change_voice_command_keep_window_open_setting,
        shortcut::change_voice_command_auto_run_setting,
        shortcut::change_voice_command_auto_run_seconds_setting,
        shortcut::change_voice_command_default_threshold_setting,
        shortcut::change_voice_commands_setting,
        shortcut::change_voice_command_use_levenshtein_setting,
        shortcut::change_voice_command_levenshtein_threshold_setting,
        shortcut::change_voice_command_use_phonetic_setting,
        shortcut::change_voice_command_phonetic_boost_setting,
        shortcut::change_voice_command_word_similarity_threshold_setting,
        shortcut::change_post_process_base_url_setting,
        shortcut::change_post_process_api_key_setting,
        shortcut::change_post_process_model_setting,
        shortcut::set_post_process_provider,
        shortcut::fetch_post_process_models,
        shortcut::fetch_llm_models,
        shortcut::add_post_process_prompt,
        shortcut::update_post_process_prompt,
        shortcut::delete_post_process_prompt,
        shortcut::set_post_process_selected_prompt,
        shortcut::add_transcription_profile,
        shortcut::update_transcription_profile,
        shortcut::delete_transcription_profile,
        shortcut::get_active_profile,
        shortcut::set_active_profile,
        shortcut::cycle_to_next_profile,
        shortcut::change_profile_switch_overlay_enabled_setting,
        shortcut::update_custom_words,
        shortcut::change_custom_words_enabled_setting,
        shortcut::change_custom_words_ngram_enabled_setting,
        shortcut::suspend_binding,
        shortcut::resume_binding,
        shortcut::change_mute_while_recording_setting,
        shortcut::change_filter_silence_setting,
        shortcut::change_ai_replace_system_prompt_setting,
        shortcut::change_ai_replace_user_prompt_setting,
        shortcut::change_ai_replace_max_chars_setting,
        shortcut::change_ai_replace_allow_no_selection_setting,
        shortcut::change_ai_replace_no_selection_system_prompt_setting,
        shortcut::change_ai_replace_allow_quick_tap_setting,
        shortcut::change_ai_replace_quick_tap_threshold_ms_setting,
        shortcut::change_ai_replace_quick_tap_system_prompt_setting,
        shortcut::set_ai_replace_provider,
        shortcut::change_ai_replace_api_key_setting,
        shortcut::change_ai_replace_model_setting,
        shortcut::set_voice_command_provider,
        shortcut::change_voice_command_api_key_setting,
        shortcut::change_voice_command_model_setting,
        shortcut::change_send_to_extension_enabled_setting,
        shortcut::change_send_to_extension_push_to_talk_setting,
        shortcut::change_send_to_extension_with_selection_system_prompt_setting,
        shortcut::change_send_to_extension_with_selection_user_prompt_setting,
        shortcut::change_send_to_extension_with_selection_enabled_setting,
        shortcut::change_send_to_extension_with_selection_push_to_talk_setting,
        shortcut::change_send_to_extension_with_selection_allow_no_voice_setting,
        shortcut::change_send_to_extension_with_selection_quick_tap_threshold_ms_setting,
        shortcut::change_send_to_extension_with_selection_no_voice_system_prompt_setting,
        shortcut::change_ai_replace_selection_push_to_talk_setting,
        shortcut::change_voice_command_push_to_talk_setting,
        shortcut::change_connector_auto_open_enabled_setting,
        shortcut::change_connector_auto_open_url_setting,
        shortcut::change_connector_port_setting,
        shortcut::change_connector_password_setting,
        shortcut::change_screenshot_capture_method_setting,
        shortcut::change_screenshot_capture_command_setting,
        shortcut::change_native_region_capture_mode_setting,
        shortcut::change_screenshot_folder_setting,
        shortcut::change_screenshot_require_recent_setting,
        shortcut::change_screenshot_timeout_seconds_setting,
        shortcut::change_screenshot_include_subfolders_setting,
        shortcut::change_screenshot_allow_no_voice_setting,
        shortcut::change_screenshot_no_voice_default_prompt_setting,
        shortcut::change_screenshot_quick_tap_threshold_ms_setting,
        shortcut::change_send_screenshot_to_extension_enabled_setting,
        shortcut::change_send_screenshot_to_extension_push_to_talk_setting,
        shortcut::change_app_language_setting,
        shortcut::change_show_tray_icon_setting,
        shortcut::change_update_checks_setting,
        shortcut::change_beta_voice_commands_enabled_setting,
        shortcut::change_voice_button_show_aot_toggle_setting,
        shortcut::change_voice_button_single_click_close_setting,
        shortcut::change_text_replacements_enabled_setting,
        shortcut::change_text_replacements_setting,
        shortcut::change_text_replacements_before_llm_setting,
        shortcut::change_text_replacement_decapitalize_after_edit_key_enabled_setting,
        shortcut::change_text_replacement_decapitalize_after_edit_key_setting,
        shortcut::change_text_replacement_decapitalize_after_edit_secondary_key_enabled_setting,
        shortcut::change_text_replacement_decapitalize_after_edit_secondary_key_setting,
        shortcut::get_text_replacement_decapitalize_overlay_state,
        shortcut::change_text_replacement_decapitalize_timeout_ms_setting,
        shortcut::change_text_replacement_decapitalize_standard_post_recording_monitor_ms_setting,
        shortcut::change_output_whitespace_leading_mode_setting,
        shortcut::change_output_whitespace_trailing_mode_setting,
        shortcut::change_sidebar_pinned_setting,
        shortcut::change_sidebar_width_setting,
        shortcut::get_language_from_os_input,
        shortcut::get_current_shortcut_engine,
        shortcut::set_shortcut_engine_setting,
        shortcut::get_tauri_incompatible_shortcuts,
        trigger_update_check,
        commands::cancel_operation,
        commands::get_app_dir_path,
        commands::get_app_settings,
        commands::get_default_settings,
        commands::get_log_dir_path,
        commands::set_log_level,
        commands::open_recordings_folder,
        commands::open_log_dir,
        commands::open_app_data_dir,
        commands::remote_stt::remote_stt_has_api_key,
        commands::remote_stt::remote_stt_set_api_key,
        commands::remote_stt::remote_stt_clear_api_key,
        commands::remote_stt::soniox_has_api_key,
        commands::remote_stt::soniox_set_api_key,
        commands::remote_stt::soniox_clear_api_key,
        commands::remote_stt::remote_stt_get_debug_dump,
        commands::remote_stt::remote_stt_clear_debug,
        commands::remote_stt::remote_stt_test_connection,
        commands::remote_stt::remote_stt_get_prompt_limit,
        commands::remote_stt::remote_stt_supports_translation,
        commands::check_apple_intelligence_available,
        commands::llm_has_stored_api_key,
        commands::models::get_available_models,
        commands::models::get_model_info,
        commands::models::download_model,
        commands::models::delete_model,
        commands::models::cancel_download,
        commands::models::set_active_model,
        commands::models::get_current_model,
        commands::models::get_transcription_model_status,
        commands::models::is_model_loading,
        commands::models::has_any_models_available,
        commands::models::has_any_models_or_downloads,
        commands::models::get_recommended_first_model,
        commands::models::get_active_gpu_vram_status,
        commands::audio::update_microphone_mode,
        commands::audio::get_microphone_mode,
        commands::audio::get_available_microphones,
        commands::audio::set_selected_microphone,
        commands::audio::get_selected_microphone,
        commands::audio::get_available_output_devices,
        commands::audio::set_selected_output_device,
        commands::audio::get_selected_output_device,
        commands::audio::play_test_sound,
        commands::audio::check_custom_sounds,
        commands::audio::set_clamshell_microphone,
        commands::audio::get_clamshell_microphone,
        commands::audio::is_recording,
        commands::audio::change_vad_threshold_setting,
        commands::transcription::set_model_unload_timeout,
        commands::transcription::get_model_load_status,
        commands::transcription::unload_model_manually,
        commands::history::get_history_entries,
        commands::history::toggle_history_entry_saved,
        commands::history::get_audio_file_path,
        commands::history::delete_history_entry,
        commands::history::update_history_limit,
        commands::history::update_recording_retention_period,
        commands::history::get_latest_history_entry,
        commands::connector::connector_get_status,
        commands::connector::connector_is_online,
        commands::connector::connector_start_server,
        commands::connector::connector_stop_server,
        commands::connector::connector_queue_message,
        commands::connector::connector_cancel_message,
        commands::region_capture::region_capture_get_data,
        commands::region_capture::region_capture_confirm,
        commands::region_capture::region_capture_cancel,
        commands::voice_command::execute_voice_command,
        commands::voice_command::test_voice_command_mock,
        commands::voice_activation_button::spawn_voice_activation_button_window,
        commands::voice_activation_button::voice_activation_button_get_push_to_talk,
        commands::voice_activation_button::voice_activation_button_get_show_aot_toggle,
        commands::voice_activation_button::voice_activation_button_get_single_click_close,
        commands::voice_activation_button::voice_activation_button_press,
        commands::voice_activation_button::voice_activation_button_release,
        commands::file_transcription::get_supported_audio_extensions,
        commands::file_transcription::transcribe_audio_file,
        commands::key_listener::key_listener_start,
        commands::key_listener::key_listener_stop,
        commands::key_listener::key_listener_is_running,
        commands::key_listener::key_listener_get_modifiers,
        commands::key_listener::key_listener_register_shortcut,
        commands::key_listener::key_listener_unregister_shortcut,
        commands::key_listener::key_listener_is_shortcut_registered,
        commands::key_listener::key_listener_get_registered_shortcuts,
        overlay::get_soniox_live_preview_state,
        overlay::get_soniox_live_preview_appearance,
        helpers::clamshell::is_laptop,
    ]);

    #[cfg(debug_assertions)] // <- Only export on non-release builds
    specta_builder
        .export(
            Typescript::default().bigint(BigIntExportBehavior::Number),
            "../src/bindings.ts",
        )
        .expect("Failed to export typescript bindings");

    // mut is required on macOS where we add the nspanel plugin
    #[allow(unused_mut)]
    let mut builder = tauri::Builder::default().plugin(
        LogBuilder::new()
            .level(log::LevelFilter::Trace) // Set to most verbose level globally
            .max_file_size(500_000)
            .rotation_strategy(RotationStrategy::KeepOne)
            .clear_targets()
            .targets([
                // Console output respects RUST_LOG environment variable
                Target::new(TargetKind::Stdout).filter({
                    let console_filter = console_filter.clone();
                    move |metadata| console_filter.enabled(metadata)
                }),
                // File logs respect the user's settings (stored in FILE_LOG_LEVEL atomic)
                Target::new(TargetKind::LogDir {
                    file_name: Some("aivorelay".into()),
                })
                .filter(|metadata| {
                    let file_level = FILE_LOG_LEVEL.load(Ordering::Relaxed);
                    metadata.level() <= level_filter_from_u8(file_level)
                }),
            ])
            .build(),
    );

    #[cfg(target_os = "macos")]
    {
        builder = builder.plugin(tauri_nspanel::init());
    }

    builder
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            show_main_window(app);
        }))
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_os::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_macos_permissions::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_store::Builder::default().build())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .plugin(tauri_plugin_autostart::init(
            MacosLauncher::LaunchAgent,
            Some(vec![]),
        ))
        .manage(Mutex::new(ShortcutToggleStates::default()))
        .manage(Mutex::new(PressTimestamps::default()))
        .manage(Mutex::new(session_manager::SessionState::default()))
        .manage(std::sync::Mutex::new(std::collections::HashSet::<String>::new()) as shortcut::RdevShortcutsSet)
        .manage(std::sync::Mutex::new(settings::ShortcutEngine::default()) as shortcut::ActiveShortcutEngine)
        .setup(move |app| {
            let settings = get_settings(&app.handle());
            let tauri_log_level: tauri_plugin_log::LogLevel = settings.log_level.into();
            let file_log_level: log::Level = tauri_log_level.into();
            // Store the file log level in the atomic for the filter to use
            FILE_LOG_LEVEL.store(file_log_level.to_level_filter() as u8, Ordering::Relaxed);
            let app_handle = app.handle().clone();

            initialize_core_logic(&app_handle);

            // Show main window only if not starting hidden
            if !settings.start_hidden {
                if let Some(main_window) = app_handle.get_webview_window("main") {
                    main_window.show().unwrap();
                    main_window.set_focus().unwrap();
                }
            }

            Ok(())
        })
        .on_window_event(|window, event| match event {
            tauri::WindowEvent::CloseRequested { api, .. } => {
                // Only the main window should be hidden-to-tray on close.
                // Auxiliary windows (e.g. voice activation button) must be allowed to close.
                if window.label() == "main" {
                    let settings = get_settings(&window.app_handle());
                    if !settings.show_tray_icon {
                        window.app_handle().exit(0);
                        return;
                    }

                    api.prevent_close();
                    let _res = window.hide();
                    #[cfg(target_os = "macos")]
                    {
                        let res = window
                            .app_handle()
                            .set_activation_policy(tauri::ActivationPolicy::Accessory);
                        if let Err(e) = res {
                            log::error!("Failed to set activation policy: {}", e);
                        }
                    }
                }
            }
            tauri::WindowEvent::ThemeChanged(theme) => {
                log::info!("Theme changed to: {:?}", theme);
                // Update tray icon to match new theme, maintaining idle state
                utils::change_tray_icon(&window.app_handle(), utils::TrayIconState::Idle);
            }
            _ => {}
        })
        .invoke_handler(specta_builder.invoke_handler())
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
