use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_clipboard_manager::ClipboardExt;
use tauri_plugin_global_shortcut::{GlobalShortcutExt, ShortcutEvent, ShortcutState};
use tracing::{debug, info, warn};

use crate::config::settings::ShortcutSettings;
use crate::AppState;

pub(crate) fn reset_input_shortcut_state(app: &AppHandle) {
    if let Some(state) = app.try_state::<AppState>() {
        if let Ok(mut pressed) = state.input_shortcut_pressed.lock() {
            *pressed = false;
        }
        if let Ok(mut last) = state.last_input_shortcut_tap.lock() {
            *last = None;
        }
        if let Ok(mut pressed_at) = state.input_shortcut_pressed_at.lock() {
            *pressed_at = None;
        }
        if let Ok(mut started_by_hold) = state.hold_started_recording.lock() {
            *started_by_hold = false;
        }
    }
}

pub(crate) fn register_global_shortcuts(
    app: &AppHandle,
    shortcuts: ShortcutSettings,
) -> Result<(), String> {
    let global = app.global_shortcut();
    global
        .unregister_all()
        .map_err(|e| format!("ショートカットの再登録に失敗しました: {}", e))?;

    reset_input_shortcut_state(app);

    let input_shortcut = crate::config::settings::validate_shortcut(&shortcuts.input_shortcut)
        .unwrap_or_else(|_| crate::config::settings::get_shortcut_settings().input_shortcut);
    let handle_for_input = app.clone();
    global
        .on_shortcut(input_shortcut.as_str(), move |_app, _shortcut, event| {
            let app_clone = handle_for_input.clone();
            tauri::async_runtime::spawn(async move {
                handle_input_shortcut(app_clone, event).await;
            });
        })
        .map_err(|e| format!("入力ショートカットの登録に失敗しました: {}", e))?;

    info!("グローバルショートカット: {}", input_shortcut);

    let os_paste_shortcut =
        crate::config::settings::validate_shortcut(&shortcuts.os_paste_shortcut)
            .unwrap_or_else(|_| crate::config::settings::get_shortcut_settings().os_paste_shortcut);
    let handle_for_os_paste = app.clone();
    global
        .on_shortcut(
            os_paste_shortcut.as_str(),
            move |_app, _shortcut, _event| {
                let handle = handle_for_os_paste.clone();
                tauri::async_runtime::spawn(async move {
                    if let Some(text) = crate::paste::resolve_latest_aux_text(&handle) {
                        if let Err(e) =
                            crate::paste::paste_text_preserving_clipboard(&handle, &text)
                        {
                            warn!("補助文字起こしペースト失敗: {}", e);
                        } else {
                            debug!("補助文字起こしをペーストしました");
                        }
                    } else {
                        debug!("補助文字起こしの履歴がありません");
                    }
                });
            },
        )
        .map_err(|e| {
            format!(
                "補助文字起こしペーストショートカットの登録に失敗しました: {}",
                e
            )
        })?;

    debug!(
        "補助文字起こしペーストショートカット登録完了: {}",
        os_paste_shortcut
    );

    Ok(())
}

fn log_runtime_error(
    app: &AppHandle,
    error_type: crate::error_log::ErrorType,
    context: &'static str,
    message: impl Into<String>,
) {
    let message = message.into();
    warn!("{}: {}", context, message);
    let _ = crate::error_log::ErrorLogManager::add_error(error_type, &message, Some(context));
    let _ = app.emit(
        "runtime-error",
        serde_json::json!({
            "context": context,
            "message": message
        }),
    );
}

fn emit_event_or_log<T: serde::Serialize + Clone>(
    app: &AppHandle,
    event_name: &'static str,
    payload: T,
    context: &'static str,
) {
    if let Err(e) = app.emit(event_name, payload) {
        log_runtime_error(
            app,
            crate::error_log::ErrorType::UnknownError,
            context,
            format!(
                "イベント送信に失敗しました: event={}, error={}",
                event_name, e
            ),
        );
    }
}

fn ensure_floating_window_visible(app: &AppHandle, context: &'static str) {
    match app.get_webview_window("floating") {
        Some(window) => {
            if let Err(e) = window.show() {
                log_runtime_error(
                    app,
                    crate::error_log::ErrorType::UnknownError,
                    context,
                    format!("floating window の show に失敗しました: {}", e),
                );
            }
        }
        None => {
            log_runtime_error(
                app,
                crate::error_log::ErrorType::UnknownError,
                context,
                "floating window が見つかりません",
            );
        }
    }
}

fn cancel_single_tap_task(state: &AppState) {
    if let Ok(mut guard) = state.pending_single_tap_task.lock() {
        if let Some(handle) = guard.take() {
            handle.abort();
        }
    }
}

fn cancel_hold_task(state: &AppState) {
    if let Ok(mut guard) = state.pending_hold_task.lock() {
        if let Some(handle) = guard.take() {
            handle.abort();
        }
    }
}

fn reset_ai_processing_state(state: &AppState) {
    if let Ok(mut guard) = state.ai_mode_active.lock() {
        *guard = false;
    }
    if let Ok(mut guard) = state.pending_selected_text.lock() {
        *guard = None;
    }
}

fn is_whisper_runtime_missing_error(message: &str) -> bool {
    message.contains("Whisperモデルが見つかりません")
        || message.contains("whisper-cli が見つかりません")
}

/// 文字起こし失敗をUIへ通知し、フローティングを閉じてAI処理状態をリセットする。
fn notify_transcription_failed_and_reset(app: &AppHandle, state: &AppState, message: &str) {
    let _ = app.emit("transcription-failed", message.to_string());
    crate::overlay::overlay_send_json(
        app,
        serde_json::json!({
            "type": "transcription-failed",
            "message": message
        }),
    );
    crate::overlay::close_floating_window_after_delay(app, 2000);
    reset_ai_processing_state(state);
}

/// 文字起こし開始後にキャンセル要求があったかを確認し、あればUIへ通知して状態をリセットする。
fn consume_cancellation(app: &AppHandle, state: &AppState, cancel_seq_at_start: u64) -> bool {
    let cancel_seq_now = state
        .transcription_cancel_seq
        .lock()
        .ok()
        .map(|g| *g)
        .unwrap_or(cancel_seq_at_start);
    if cancel_seq_now == cancel_seq_at_start {
        return false;
    }
    let _ = app.emit("transcription-cancelled", ());
    crate::overlay::overlay_send_json(
        app,
        serde_json::json!({ "type": "transcription-cancelled" }),
    );
    reset_ai_processing_state(state);
    true
}

/// AIモード（選択テキストあり）のとき、音声テキストを編集指示として選択テキストへ適用する。
async fn apply_voice_edit_to_selection(
    selected_text_for_ai: Option<&str>,
    voice_text: &str,
) -> Result<String, String> {
    let selected_text = selected_text_for_ai
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
        .ok_or_else(|| "選択テキストを取得できませんでした".to_string())?;
    let api_key =
        crate::config::get_api_key().map_err(|e| format!("APIキーの取得に失敗: {}", e))?;
    let ai_model = crate::config::get_ai_model();
    crate::transcription::process_selected_text_with_voice(
        &selected_text,
        voice_text,
        &api_key,
        &ai_model,
    )
    .await
    .map_err(|e| e.to_string())
}

fn emit_whisper_runtime_missing_guidance(app: &AppHandle) {
    let message = crate::commands::WHISPER_RUNTIME_MISSING_GUIDANCE_MESSAGE.to_string();
    let _ = app.emit("whisper-runtime-missing", message.clone());
    crate::overlay::overlay_send_json(
        app,
        serde_json::json!({
            "type": "status",
            "status": "error",
            "message": message,
        }),
    );
}

async fn handle_input_shortcut(app: AppHandle, event: ShortcutEvent) {
    let state = app.state::<AppState>();
    let last_release_arc = state.last_input_shortcut_tap.clone();
    let pressed_flag_arc = state.input_shortcut_pressed.clone();
    let pressed_at_arc = state.input_shortcut_pressed_at.clone();
    let hold_started_arc = state.hold_started_recording.clone();
    let pending_hold_arc = state.pending_hold_task.clone();

    const TAP_MAX_MS: u64 = 220;
    const DOUBLE_WINDOW_MS: u64 = 450;
    const HOLD_THRESHOLD_MS: u64 = 220;

    match event.state {
        ShortcutState::Pressed => {
            // 既に押下状態なら無視（OSからのリピート防止）
            {
                let Ok(mut pressed) = pressed_flag_arc.lock() else {
                    warn!("input_shortcut_pressed lock poisoned on press");
                    return;
                };
                if *pressed {
                    return;
                }
                *pressed = true;
            }

            // 録音中なら即停止（従来動作）
            let is_recording_now = match state.is_recording.lock() {
                Ok(guard) => *guard,
                Err(_) => {
                    warn!("is_recording lock poisoned on press");
                    return;
                }
            };
            if is_recording_now {
                cancel_single_tap_task(&state);
                cancel_hold_task(&state);
                if let Ok(mut last) = last_release_arc.lock() {
                    *last = None;
                }
                if let Ok(mut started_by_hold) = hold_started_arc.lock() {
                    *started_by_hold = false;
                }
                if let Ok(mut pressed_at) = pressed_at_arc.lock() {
                    *pressed_at = Some(std::time::Instant::now());
                }
                handle_shortcut_toggle(app).await;
                return;
            }

            if let Ok(mut pressed_at) = pressed_at_arc.lock() {
                *pressed_at = Some(std::time::Instant::now());
            }
            if let Ok(mut started_by_hold) = hold_started_arc.lock() {
                *started_by_hold = false;
            }

            cancel_hold_task(&state);

            let app_for_hold = app.clone();
            let pressed_flag_for_hold = pressed_flag_arc.clone();
            let hold_started_for_hold = hold_started_arc.clone();
            let pending_hold_for_hold = pending_hold_arc.clone();

            let hold_task = tauri::async_runtime::spawn(async move {
                tokio::time::sleep(std::time::Duration::from_millis(HOLD_THRESHOLD_MS)).await;
                let still_pressed = {
                    match pressed_flag_for_hold.lock() {
                        Ok(guard) => *guard,
                        Err(_) => {
                            warn!("input_shortcut_pressed lock poisoned on hold");
                            false
                        }
                    }
                };
                if still_pressed {
                    let state = app_for_hold.state::<AppState>();
                    let is_recording_now = match state.is_recording.lock() {
                        Ok(guard) => *guard,
                        Err(_) => {
                            warn!("is_recording lock poisoned on hold");
                            true
                        }
                    };
                    if !is_recording_now {
                        if let Ok(mut started) = hold_started_for_hold.lock() {
                            *started = true;
                        }
                        handle_shortcut_toggle(app_for_hold).await;
                    }
                }
                if let Ok(mut guard) = pending_hold_for_hold.lock() {
                    guard.take();
                }
            });

            if let Ok(mut guard) = pending_hold_arc.lock() {
                *guard = Some(hold_task);
            }
        }
        ShortcutState::Released => {
            // 押下フラグが立っていない場合は無視
            {
                let Ok(mut pressed) = pressed_flag_arc.lock() else {
                    warn!("input_shortcut_pressed lock poisoned on release");
                    return;
                };
                if !*pressed {
                    return;
                }
                *pressed = false;
            }

            cancel_hold_task(&state);

            let now = std::time::Instant::now();
            let press_duration_ms = match pressed_at_arc.lock() {
                Ok(mut guard) => guard
                    .take()
                    .map(|t| now.duration_since(t).as_millis() as u64),
                Err(_) => {
                    warn!("input_shortcut_pressed_at lock poisoned on release");
                    None
                }
            };

            let started_by_hold = match hold_started_arc.lock() {
                Ok(mut guard) => {
                    let v = *guard;
                    *guard = false;
                    v
                }
                Err(_) => {
                    warn!("hold_started_recording lock poisoned on release");
                    false
                }
            };

            if started_by_hold {
                // ホールドで開始した録音はリリースで停止
                if state
                    .is_recording
                    .lock()
                    .map(|guard| *guard)
                    .unwrap_or(false)
                {
                    handle_shortcut_toggle(app).await;
                }
                return;
            }

            // ダブルタップ判定（リリース時点）
            let is_double_tap = match last_release_arc.lock() {
                Ok(mut last) => {
                    let res = if let Some(prev) = *last {
                        now.duration_since(prev).as_millis() as u64 <= DOUBLE_WINDOW_MS
                    } else {
                        false
                    };
                    if res {
                        *last = None;
                    } else {
                        *last = Some(now);
                    }
                    res
                }
                Err(_) => {
                    warn!("last_input_shortcut_tap lock poisoned on release");
                    false
                }
            };

            if is_double_tap {
                handle_shortcut_toggle(app).await;
                return;
            }

            // シングルタップ: ペースト。ただし長押し（tap_max超え）は無視
            if let Some(d) = press_duration_ms {
                if d > TAP_MAX_MS {
                    return;
                }
            }

            cancel_single_tap_task(&state);

            if let Some(text) = crate::paste::resolve_latest_llm_text(&app) {
                match crate::paste::paste_text_preserving_clipboard(&app, &text) {
                    Ok(_) => {
                        debug!("シングルタップで前回の文字起こしをペースト");
                        let _ = app.emit("paste-completed", ());
                        crate::overlay::overlay_send_json(
                            &app,
                            serde_json::json!({
                                "type": "paste-completed"
                            }),
                        );
                    }
                    Err(e) => {
                        warn!("シングルタップペーストエラー: {}", e);

                        // エラーログに記録
                        let _ = crate::error_log::ErrorLogManager::add_error(
                            crate::error_log::ErrorType::UnknownError,
                            &e,
                            Some("paste_last_transcription_single_tap"),
                        );

                        let _ = app.emit("paste-failed", &e);
                        crate::overlay::overlay_send_json(
                            &app,
                            serde_json::json!({
                                "type": "paste-failed",
                                "message": e
                            }),
                        );
                    }
                }
            } else {
                debug!("シングルタップ: 直近の文字起こしがないため何もしません");
            }
        }
    }
}

// ショートカットハンドラ（トグル録音）
async fn handle_shortcut_toggle(app: AppHandle) {
    let state = app.state::<AppState>();
    cancel_single_tap_task(&state);
    if let Ok(mut last) = state.last_input_shortcut_tap.lock() {
        *last = None;
    }
    let is_recording = match state.is_recording.lock() {
        Ok(guard) => *guard,
        Err(_) => {
            warn!("recording state lock poisoned");
            return;
        }
    };

    if is_recording {
        // 録音停止 → 文字起こし → ペースト
        info!("録音停止");

        let path_result = {
            match state.recorder.lock() {
                Ok(mut recorder) => recorder.stop_recording(),
                Err(_) => {
                    warn!("recorder lock poisoned while stopping");
                    return;
                }
            }
        };

        match path_result {
            Ok(path) => {
                let path_str = path.to_string_lossy().to_string();
                if let Ok(mut guard) = state.is_recording.lock() {
                    *guard = false;
                } else {
                    warn!("is_recording lock poisoned while stopping");
                    return;
                }
                if let Ok(mut guard) = state.recording_path.lock() {
                    *guard = Some(path_str.clone());
                } else {
                    warn!("recording_path lock poisoned while stopping");
                    return;
                }

                // 録音時間が短い場合は処理しない
                let min_input_ms = crate::config::settings::get_min_input_duration_ms();
                let elapsed_ms = state
                    .recording_started_at
                    .lock()
                    .ok()
                    .and_then(|mut guard| guard.take())
                    .map(|start| start.elapsed().as_millis() as u64);
                let recording_duration_ms = elapsed_ms;
                if let Some(duration_ms) = elapsed_ms {
                    if duration_ms <= min_input_ms {
                        info!(
                            "録音が短いためスキップ: {}ms (閾値 {}ms)",
                            duration_ms, min_input_ms
                        );
                        if let Ok(mut pending) = state.pending_os_text.lock() {
                            *pending = None;
                        }
                        if let Ok(mut latest) = state.latest_os_text.lock() {
                            *latest = None;
                        }
                        let _ = app.emit("recording-skipped", ());
                        crate::overlay::overlay_send_json(
                            &app,
                            serde_json::json!({ "type": "hide" }),
                        );
                        reset_ai_processing_state(&state);
                        return;
                    }
                }

                // フロントエンドに通知
                let _ = app.emit("recording-stopped", &path_str);
                crate::overlay::overlay_send_json(
                    &app,
                    serde_json::json!({
                        "type": "recording-stopped"
                    }),
                );

                // 録音終了時点のOS標準入力を確定（履歴保存はLLM完了時に行う）
                if let Ok(mut pending) = state.pending_os_text.lock() {
                    if let Some(text) = pending.take() {
                        let text = text.trim().to_string();
                        if !text.is_empty() {
                            if let Ok(mut latest) = state.latest_os_text.lock() {
                                *latest = Some(text.clone());
                            }
                            // 履歴保存は削除（LLM完了時に統合保存）
                        }
                    }
                }

                // 文字起こし実行前に少し待機（ファイル書き込み完了を確実にするため）
                tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

                // 文字起こし開始
                info!("文字起こし中...");
                let cancel_seq_at_start = state
                    .transcription_cancel_seq
                    .lock()
                    .ok()
                    .map(|g| *g)
                    .unwrap_or(0);
                let stt_provider = crate::config::get_stt_provider();
                let start_time = std::time::Instant::now();
                let selected_text_for_ai = match crate::paste::detect_selected_text(&app) {
                    Ok(value) => value,
                    Err(e) => {
                        warn!("選択テキスト取得エラー: {}", e);
                        let _ = crate::error_log::ErrorLogManager::add_error(
                            crate::error_log::ErrorType::UnknownError,
                            &e,
                            Some("detect_selected_text_after_recording"),
                        );
                        notify_transcription_failed_and_reset(&app, &state, &e);
                        return;
                    }
                };
                let ai_mode_active = selected_text_for_ai
                    .as_ref()
                    .map(|v| !v.trim().is_empty())
                    .unwrap_or(false);
                if let Ok(mut guard) = state.ai_mode_active.lock() {
                    *guard = ai_mode_active;
                }
                if let Ok(mut guard) = state.pending_selected_text.lock() {
                    *guard = selected_text_for_ai.clone();
                }

                let dispatch = crate::transcription::dispatch::transcribe_with_provider(
                    stt_provider,
                    &path_str,
                    recording_duration_ms,
                )
                .await;
                let whisper_text_for_history = dispatch.whisper_text;
                let mut deferred_error_logs = dispatch.deferred_error_logs;
                let transcription_result = dispatch.result;

                match transcription_result {
                    Ok(text) => {
                        if consume_cancellation(&app, &state, cancel_seq_at_start) {
                            info!("文字起こしキャンセル");
                            return;
                        }
                        let duration_ms = start_time.elapsed().as_millis() as u64;
                        info!("完了");

                        let final_text = if ai_mode_active {
                            match apply_voice_edit_to_selection(
                                selected_text_for_ai.as_deref(),
                                &text,
                            )
                            .await
                            {
                                Ok(v) => v,
                                Err(e) => {
                                    notify_transcription_failed_and_reset(&app, &state, &e);
                                    return;
                                }
                            }
                        } else {
                            text.clone()
                        };

                        // OS標準音声入力を取得
                        let os_text_for_history = app
                            .state::<AppState>()
                            .latest_os_text
                            .lock()
                            .ok()
                            .and_then(|g| g.clone());
                        let prompt_text_for_history = if ai_mode_active {
                            selected_text_for_ai
                                .clone()
                                .map(|selected| {
                                    format!(
                                        "[AI_PROMPT]\n{}\n[SELECTED_TEXT]\n{}",
                                        text.trim(),
                                        selected.trim()
                                    )
                                })
                                .filter(|v| !v.trim().is_empty())
                        } else {
                            os_text_for_history.clone()
                        };

                        // latest_llm_textの更新
                        if let Ok(mut latest) = app.state::<AppState>().latest_llm_text.lock() {
                            *latest = Some(final_text.clone());
                        }

                        let _ = app.emit("transcription-completed", &final_text);
                        crate::overlay::overlay_send_json(
                            &app,
                            serde_json::json!({
                                "type": "transcription-completed",
                                "text": final_text
                            }),
                        );

                        // フローティングウィンドウは Swift 側で一定時間後に自動非表示
                        // ペースト処理前に少し待機（他のアプリがフォーカスを得る時間）
                        tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;

                        // 自動ペースト（クリップボード復元あり）
                        debug!("ペースト処理を開始");
                        match crate::paste::paste_text_preserving_clipboard(&app, &final_text) {
                            Ok(_) => {
                                info!("完了");
                                let _ = app.emit("paste-completed", ());
                                crate::overlay::overlay_send_json(
                                    &app,
                                    serde_json::json!({
                                        "type": "paste-completed"
                                    }),
                                );
                            }
                            Err(e) => {
                                warn!("ペーストエラー: {}", e);
                                match app.clipboard().write_text(final_text.clone()) {
                                    Ok(_) => {
                                        let _ = app.emit("copy-completed", ());
                                        crate::overlay::overlay_send_json(
                                            &app,
                                            serde_json::json!({
                                                "type": "copy-completed"
                                            }),
                                        );
                                    }
                                    Err(copy_error) => {
                                        let message =
                                            format!("{} / copy fallback失敗: {}", e, copy_error);
                                        let _ = app.emit("paste-failed", &message);
                                        crate::overlay::overlay_send_json(
                                            &app,
                                            serde_json::json!({
                                                "type": "paste-failed",
                                                "message": message
                                            }),
                                        );
                                        let paste_error = message.clone();
                                        tokio::task::spawn_blocking(move || {
                                            let _ = crate::error_log::ErrorLogManager::add_error(
                                                crate::error_log::ErrorType::UnknownError,
                                                &paste_error,
                                                Some("paste_after_transcription"),
                                            );
                                        });
                                    }
                                }
                            }
                        }

                        // ユーザーへの提供完了後に、履歴保存と遅延ログ書き込みをバックグラウンド実行
                        let text_for_history = final_text.clone();
                        let prompt_text_for_history_bg = prompt_text_for_history.clone();
                        let deferred_logs = std::mem::take(&mut deferred_error_logs);
                        tokio::task::spawn_blocking(move || {
                            if let Err(e) = crate::history::HistoryManager::add_entry_with_dictation(
                                &text_for_history,
                                prompt_text_for_history_bg.as_deref(),
                                whisper_text_for_history.as_deref(),
                                Some(stt_provider.as_str()),
                                duration_ms,
                            ) {
                                debug!("履歴保存エラー: {}", e);
                            }

                            for (error_type, message, context) in deferred_logs {
                                let _ = crate::error_log::ErrorLogManager::add_error(
                                    error_type,
                                    &message,
                                    Some(&context),
                                );
                            }
                        });
                        reset_ai_processing_state(&state);
                    }
                    Err(e) => {
                        if consume_cancellation(&app, &state, cancel_seq_at_start) {
                            info!("文字起こしエラー応答はキャンセル済みのため破棄");
                            return;
                        }
                        let duration_ms = start_time.elapsed().as_millis() as u64;
                        warn!("文字起こしエラー: {}", e);

                        let error_type = if e.contains("APIキー") {
                            crate::error_log::ErrorType::ConfigError
                        } else {
                            crate::error_log::ErrorType::ApiError
                        };

                        // OS標準音声入力を取得
                        let os_text_for_history = app
                            .state::<AppState>()
                            .latest_os_text
                            .lock()
                            .ok()
                            .and_then(|g| g.clone());

                        let _ = app.emit("transcription-failed", e.clone());
                        crate::overlay::overlay_send_json(
                            &app,
                            serde_json::json!({
                                "type": "transcription-failed",
                                "message": e
                            }),
                        );

                        if is_whisper_runtime_missing_error(&e) {
                            emit_whisper_runtime_missing_guidance(&app);
                            let _ = crate::commands::open_settings_tab_internal(
                                app.clone(),
                                "onboarding",
                            );
                        } else {
                            // エラー発生時はウィンドウを自動で閉じる
                            crate::overlay::close_floating_window_after_delay(&app, 2000);
                        }

                        // エラー記録はユーザー通知後にバックグラウンド実行
                        let error_message = e.clone();
                        let os_text_for_history_bg = os_text_for_history.clone();
                        tokio::task::spawn_blocking(move || {
                            let _ = crate::error_log::ErrorLogManager::add_error(
                                error_type,
                                &error_message,
                                Some("transcription"),
                            );

                            if let Some(os_text) = os_text_for_history_bg.as_deref() {
                                let error_text =
                                    format!("(エラー: 文字起こし失敗 - {})", error_message);
                                if let Err(e) =
                                    crate::history::HistoryManager::add_entry_with_dictation(
                                        &error_text,
                                        Some(os_text),
                                        None,
                                        Some(stt_provider.as_str()),
                                        duration_ms,
                                    )
                                {
                                    debug!("履歴保存エラー: {}", e);
                                }
                            }
                        });
                        reset_ai_processing_state(&state);
                    }
                }
            }
            Err(e) => {
                warn!("録音停止エラー: {}", e);
                if let Ok(mut guard) = state.is_recording.lock() {
                    *guard = false;
                }
                if let Ok(mut started_at) = state.recording_started_at.lock() {
                    *started_at = None;
                }
                if let Ok(mut recording_path) = state.recording_path.lock() {
                    *recording_path = None;
                }

                // エラーログに記録
                let _ = crate::error_log::ErrorLogManager::add_error(
                    crate::error_log::ErrorType::AudioError,
                    &e.to_string(),
                    Some("stop_recording"),
                );

                let _ = app.emit("recording-error", e.to_string());
                crate::overlay::overlay_send_json(
                    &app,
                    serde_json::json!({
                        "type": "recording-error",
                        "message": e.to_string()
                    }),
                );

                // エラー発生時はウィンドウを自動で閉じる
                crate::overlay::close_floating_window_after_delay(&app, 2000);
                reset_ai_processing_state(&state);
            }
        }
    } else {
        // 録音開始
        info!("録音開始");
        reset_ai_processing_state(&state);

        // 表示失敗の切り分けを容易にするため、バックエンド側でも表示を先に試行する
        ensure_floating_window_visible(
            &app,
            "handle_shortcut_toggle/start/ensure_floating_window_visible",
        );
        // 録音初期化前に先行イベントを送って、フローティング表示遅延を防ぐ
        emit_event_or_log(
            &app,
            "recording-starting",
            (),
            "handle_shortcut_toggle/start/emit_recording_starting",
        );
        crate::overlay::overlay_send_json(
            &app,
            serde_json::json!({
                "type": "recording-starting"
            }),
        );

        let result = {
            match state.recorder.lock() {
                Ok(mut recorder) => recorder.start_recording(app.clone()),
                Err(_) => {
                    warn!("recorder lock poisoned while starting");
                    return;
                }
            }
        };

        match result {
            Ok(_) => {
                if let Ok(mut guard) = state.is_recording.lock() {
                    *guard = true;
                } else {
                    warn!("is_recording lock poisoned while starting");
                    return;
                }
                if let Ok(mut started_at) = state.recording_started_at.lock() {
                    *started_at = Some(std::time::Instant::now());
                }
                emit_event_or_log(
                    &app,
                    "recording-started",
                    (),
                    "handle_shortcut_toggle/start/emit_recording_started",
                );
                crate::overlay::overlay_send_json(
                    &app,
                    serde_json::json!({
                        "type": "recording-started"
                    }),
                );
            }
            Err(e) => {
                warn!("録音開始エラー: {}", e);

                // エラーログに記録
                let _ = crate::error_log::ErrorLogManager::add_error(
                    crate::error_log::ErrorType::AudioError,
                    &e.to_string(),
                    Some("start_recording"),
                );

                emit_event_or_log(
                    &app,
                    "recording-error",
                    e.to_string(),
                    "handle_shortcut_toggle/start/emit_recording_error",
                );
                crate::overlay::overlay_send_json(
                    &app,
                    serde_json::json!({
                        "type": "recording-error",
                        "message": e.to_string()
                    }),
                );

                // エラー発生時はウィンドウを自動で閉じる
                crate::overlay::close_floating_window_after_delay(&app, 2000);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::is_whisper_runtime_missing_error;

    #[test]
    fn whisper_runtime_missing_when_model_not_found() {
        assert!(is_whisper_runtime_missing_error(
            "Whisperモデルが見つかりません: /tmp/model.bin"
        ));
    }

    #[test]
    fn whisper_runtime_missing_when_cli_not_found() {
        assert!(is_whisper_runtime_missing_error(
            "whisper-cli が見つかりません。確認した候補: PATH:whisper-cli"
        ));
    }

    #[test]
    fn whisper_runtime_missing_false_for_unrelated_error() {
        assert!(!is_whisper_runtime_missing_error(
            "Gemini API エラー: unauthorized"
        ));
    }
}
