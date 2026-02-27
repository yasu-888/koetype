#![cfg_attr(target_os = "macos", allow(unexpected_cfgs))]

mod audio;
mod clipboard;
mod commands;
mod config;
mod error_log;
mod history;
mod overlay;
mod paste;
mod shortcut;
mod transcription;
mod tray;
mod util;

use audio::AudioRecorder;
use std::sync::{Arc, Mutex, MutexGuard};
use tauri::{Emitter, Manager};
use tracing::{debug, info, warn};

// アプリケーション状態
pub(crate) struct AppState {
    recorder: Mutex<AudioRecorder>,
    is_recording: Arc<Mutex<bool>>,
    recording_path: Arc<Mutex<Option<String>>>,
    recording_started_at: Arc<Mutex<Option<std::time::Instant>>>,
    latest_llm_text: Arc<Mutex<Option<String>>>,
    latest_os_text: Arc<Mutex<Option<String>>>,
    pending_os_text: Arc<Mutex<Option<String>>>,
    pending_selected_text: Arc<Mutex<Option<String>>>,
    ai_mode_active: Arc<Mutex<bool>>,
    #[cfg_attr(not(target_os = "macos"), allow(dead_code))]
    overlay: Mutex<Option<overlay::OverlayProcess>>,
    last_input_shortcut_tap: Arc<Mutex<Option<std::time::Instant>>>,
    pending_single_tap_task: Arc<Mutex<Option<tauri::async_runtime::JoinHandle<()>>>>,
    input_shortcut_pressed: Arc<Mutex<bool>>,
    input_shortcut_pressed_at: Arc<Mutex<Option<std::time::Instant>>>,
    hold_started_recording: Arc<Mutex<bool>>,
    pending_hold_task: Arc<Mutex<Option<tauri::async_runtime::JoinHandle<()>>>>,
    transcription_cancel_seq: Arc<Mutex<u64>>,
    last_paste_attempt: Arc<Mutex<paste::LastPasteAttempt>>,
}

#[derive(serde::Serialize, Clone, Debug)]
pub(crate) struct FeedbackSettings {
    pub(crate) notification_mode: config::settings::FeedbackMode,
    pub(crate) sound_mode: config::settings::FeedbackMode,
}

pub(crate) fn lock_or_string_error<'a, T>(
    mutex: &'a Mutex<T>,
    lock_name: &'static str,
) -> Result<MutexGuard<'a, T>, String> {
    mutex
        .lock()
        .map_err(|_| format!("内部状態のロックに失敗しました: {}", lock_name))
}

fn should_prompt_whisper_runtime_setup() -> bool {
    let stt_provider = crate::config::get_stt_provider();
    let whisper_provider = matches!(
        stt_provider,
        crate::config::settings::SttProvider::Whisper
            | crate::config::settings::SttProvider::Hybrid
            | crate::config::settings::SttProvider::Collaborate
    );
    if !whisper_provider {
        return false;
    }
    let status = crate::transcription::whisper_local::inspect_whisper_runtime_status("large-v3-turbo");
    status.cli_path.is_none() || status.model_path.is_none()
}

fn prompt_whisper_runtime_setup(app_handle: &tauri::AppHandle) {
    if let Err(e) = commands::open_settings_tab_internal(app_handle.clone(), "onboarding") {
        warn!("Whisper未導入時の設定画面表示に失敗: {}", e);
    }
    let guidance = commands::WHISPER_RUNTIME_MISSING_GUIDANCE_MESSAGE.to_string();
    let _ = app_handle.emit("whisper-runtime-missing", guidance.clone());
    overlay::overlay_send_json(
        app_handle,
        serde_json::json!({
            "type": "status",
            "status": "error",
            "message": guidance,
        }),
    );
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // ロギングの初期化
    tracing_subscriber::fmt::init();

    #[allow(unused_mut)]
    let mut builder = tauri::Builder::default();
    #[cfg(not(debug_assertions))]
    {
        builder = builder.plugin(tauri_plugin_single_instance::init(|app, _argv, _cwd| {
            warn!("既存インスタンスがあるため新規起動要求を既存側で処理します");
            if let Some(window) = app.get_webview_window("settings") {
                let _ = window.show();
                let _ = window.set_focus();
            }
        }));
    }
    #[cfg(debug_assertions)]
    {
        info!("debug build: single-instance plugin disabled");
    }

    builder
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .setup(|app| {
            #[cfg(target_os = "macos")]
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);

            let runtime_exe = std::env::current_exe()
                .map(|p| p.display().to_string())
                .unwrap_or_else(|_| "unknown".to_string());
            info!("runtime executable: {}", runtime_exe);
            info!("bundle identifier (config): {}", app.config().identifier);
            #[cfg(target_os = "macos")]
            {
                let ax_trusted = clipboard::injector::is_macos_accessibility_trusted();
                if ax_trusted {
                    info!("macOS accessibility trusted: true");
                } else {
                    let trusted_after_prompt =
                        clipboard::injector::request_macos_accessibility_prompt();
                    warn!(
                        "macOS accessibility NOT trusted for this binary. prompt_after={} \
                        binary={} \
                        dev ビルドの場合は `pnpm macos:dev-permissions` を実行して \
                        System Preferences > Privacy > Accessibility で権限を付与してください。",
                        trusted_after_prompt,
                        runtime_exe
                    );
                }
            }

            // トレイアイコンの設定
            tray::setup_tray(app)?;

            if should_prompt_whisper_runtime_setup() {
                let app_handle = app.handle().clone();
                prompt_whisper_runtime_setup(&app_handle);
                tauri::async_runtime::spawn(async move {
                    tokio::time::sleep(std::time::Duration::from_millis(900)).await;
                    if should_prompt_whisper_runtime_setup() {
                        prompt_whisper_runtime_setup(&app_handle);
                    }
                });
            }

            // フローティングウィンドウを画面中央下部に配置
            if let Some(window) = app.get_webview_window("floating") {
                let window_ = window.clone();
                let app_handle_for_pos = app.handle().clone();
                // ウィンドウの初期化時間を待つために少し遅延させる
                std::thread::spawn(move || {
                    std::thread::sleep(std::time::Duration::from_millis(100));
                    let window_for_run = window_.clone();
                    let window_for_closure = window_.clone();
                    let app_handle_for_closure = app_handle_for_pos.clone();
                    let _ = window_for_run.run_on_main_thread(move || {
                        let window = window_for_closure;
                        // macOS の NSPanel(原作) は常にメインスクリーンに張り付くため、基準を primary_monitor に寄せる
                        let monitor = window
                            .primary_monitor()
                            .ok()
                            .flatten()
                            .or_else(|| window.current_monitor().ok().flatten());
                        if let Some(monitor) = monitor {
                            let scale_factor: f64 = monitor.scale_factor();
                            let work_area = monitor.work_area();
                            let work_size = work_area.size.to_logical::<f64>(scale_factor);
                            let work_pos = work_area.position.to_logical::<f64>(scale_factor);
                            let window_size =
                                window.outer_size().unwrap_or_default().to_logical::<f64>(scale_factor);

                            // 原作(KoeType)と同じロジック：利用可能領域(work area)の中央、下から余白を空ける
                            const FLOATING_BOTTOM_MARGIN: f64 = 56.0;
                            let x = work_pos.x + (work_size.width - window_size.width) / 2.0;
                            let y = work_pos.y + work_size.height - window_size.height - FLOATING_BOTTOM_MARGIN;

                            debug!("基準モニター: {:?}", monitor.name());
                            debug!("作業領域(論理): pos=({}, {}), size={}x{}", work_pos.x, work_pos.y, work_size.width, work_size.height);
                            debug!(
                                "ウィンドウサイズ(論理): {}x{}, 配置先: x={}, y={}",
                                window_size.width, window_size.height, x, y
                            );

                            if let Err(e) = window.set_position(tauri::LogicalPosition { x, y }) {
                                warn!("位置設定に失敗: {}", e);
                            }

                            // Swift オーバーレイにも同じ x を通知（複数回送って確実に適用）
                            let app_for_send = app_handle_for_closure.clone();
                            std::thread::spawn(move || {
                                for _ in 0..5 {
                                    overlay::overlay_send_json(
                                        &app_for_send,
                                        serde_json::json!({ "type": "position", "x": x }),
                                    );
                                    std::thread::sleep(std::time::Duration::from_millis(50));
                                }
                            });

                            // 実際に OS が置いた最終位置もログする
                            if let Ok(pos) = window.outer_position() {
                                let pos = pos.to_logical::<f64>(scale_factor);
                                debug!("実際のウィンドウ位置(論理): x={}, y={}", pos.x, pos.y);
                            }

                            #[cfg(target_os = "macos")]
                            {
                                overlay::configure_macos_floating_window(&window);
                            }

                            debug!("フローティングウィンドウを配置完了（HUDモード: CanJoinAllSpaces + FullScreenAuxiliary）");
                        }
                    });
                });
            }

            #[cfg(target_os = "macos")]
            {
                overlay::apply_platform_floating_ui(&app.handle());
            }

            // グローバルショートカット登録
            let shortcut_settings = config::settings::get_shortcut_settings();
            if let Err(e) = shortcut::register_global_shortcuts(&app.handle(), shortcut_settings) {
                return Err(Box::new(std::io::Error::new(std::io::ErrorKind::Other, e)));
            }

            info!("初期化完了");
            info!("待機中...");

            Ok(())
        })
        .manage(AppState {
            recorder: Mutex::new(AudioRecorder::new()),
            is_recording: Arc::new(Mutex::new(false)),
            recording_path: Arc::new(Mutex::new(None)),
            recording_started_at: Arc::new(Mutex::new(None)),
            latest_llm_text: Arc::new(Mutex::new(None)),
            latest_os_text: Arc::new(Mutex::new(None)),
            pending_os_text: Arc::new(Mutex::new(None)),
            pending_selected_text: Arc::new(Mutex::new(None)),
            ai_mode_active: Arc::new(Mutex::new(false)),
            overlay: Mutex::new(None),
            last_input_shortcut_tap: Arc::new(Mutex::new(None)),
            pending_single_tap_task: Arc::new(Mutex::new(None)),
            input_shortcut_pressed: Arc::new(Mutex::new(false)),
            input_shortcut_pressed_at: Arc::new(Mutex::new(None)),
            hold_started_recording: Arc::new(Mutex::new(false)),
        pending_hold_task: Arc::new(Mutex::new(None)),
        transcription_cancel_seq: Arc::new(Mutex::new(0)),
        last_paste_attempt: Arc::new(Mutex::new(paste::LastPasteAttempt::default())),
    })
        .invoke_handler(tauri::generate_handler![
            commands::set_api_key,
            commands::get_api_key,
            commands::copy_text,
            commands::probe_winrt_speech_languages,
            commands::probe_sapi_recognizers,
            commands::log_frontend_event,
            commands::get_model,
            commands::get_ai_model,
            commands::set_model,
            commands::set_ai_model,
            commands::get_whisper_model_path,
            commands::set_whisper_model_path,
            commands::pick_whisper_model_path,
            commands::get_whisper_model_paths,
            commands::set_whisper_model_paths,
            commands::install_whisper_model,
            commands::get_stt_provider,
            commands::set_stt_provider,
            commands::hide_floating_ui,
            commands::force_kill_floating_overlay,
            commands::open_terminal_install_whisper,
            commands::open_settings_window,
            commands::get_onboarding_status,
            commands::set_onboarding_ack_unnotarized,
            commands::mark_onboarding_completed,
            commands::run_permissions_probe,
            commands::open_macos_privacy_page,
            commands::check_for_updates,
            commands::is_window_occluded,
            commands::get_feedback_settings,
            commands::set_sound_setting,
            commands::get_input_delivery_mode,
            commands::set_input_delivery_mode,
            commands::get_mic_sensitivity,
            commands::set_mic_sensitivity,
            commands::get_wave_motion_scale,
            commands::set_wave_motion_scale,
            commands::cancel_transcription,
            commands::test_gemini_connection,
            commands::list_audio_devices,
            commands::set_audio_device,
            commands::get_selected_audio_device,
            commands::get_language,
            commands::set_language,
            commands::get_min_input_duration_ms,
            commands::set_min_input_duration_ms,
            commands::get_hybrid_threshold_ms,
            commands::set_hybrid_threshold_ms,
            commands::get_shortcut_settings,
            commands::set_shortcut_settings,
            commands::pause_shortcuts,
            commands::resume_shortcuts,
            commands::get_history,
            commands::clear_history,
            commands::get_stats_snapshot,
            commands::get_daily_usage,
            commands::delete_history_item,
            commands::get_user_dictionary,
            commands::add_dictionary_entry,
            commands::remove_dictionary_entry,
            commands::clear_dictionary,
            commands::get_error_log,
            commands::clear_error_log,
            paste::diagnose_paste_runtime,
            paste::run_paste_self_test,
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|_app, _event| {
            #[cfg(target_os = "macos")]
            match _event {
                tauri::RunEvent::ExitRequested { .. } | tauri::RunEvent::Exit => {
                    overlay::shutdown_overlay(_app);
                }
                _ => {}
            }
        });
}
