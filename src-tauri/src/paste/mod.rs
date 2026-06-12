use tauri::{AppHandle, Manager};
use tracing::warn;

#[cfg(not(target_os = "macos"))]
use tauri_plugin_clipboard_manager::ClipboardExt;

#[derive(Clone, Debug, Default)]
pub(crate) struct LastPasteAttempt {
    pub(crate) frontmost_before: Option<String>,
    pub(crate) frontmost_after: Option<String>,
    pub(crate) method: Option<String>,
    pub(crate) error: Option<String>,
    pub(crate) restore_policy: Option<String>,
    pub(crate) elapsed_ms: Option<u128>,
    pub(crate) applescript_status_code: Option<i32>,
    pub(crate) applescript_stderr: Option<String>,
    pub(crate) enigo_error: Option<String>,
    pub(crate) used_fallback: Option<bool>,
    pub(crate) delivery_mode: Option<String>,
    pub(crate) snapshot_taken: Option<bool>,
    pub(crate) restore_attempted: Option<bool>,
    pub(crate) restore_ok: Option<bool>,
    pub(crate) transient_markers_applied: Option<bool>,
    pub(crate) type_error: Option<String>,
}

#[derive(Clone, serde::Serialize)]
pub(crate) struct PasteRuntimeDiag {
    pub(crate) executable_path: String,
    pub(crate) bundle_identifier: String,
    pub(crate) codesign_cdhash: Option<String>,
    pub(crate) ax_trusted: bool,
    pub(crate) frontmost_app_bundle_id: Option<String>,
    pub(crate) frontmost_before_paste: Option<String>,
    pub(crate) frontmost_after_paste: Option<String>,
    pub(crate) last_paste_method: Option<String>,
    pub(crate) last_paste_error: Option<String>,
    pub(crate) last_restore_policy: Option<String>,
    pub(crate) last_paste_elapsed_ms: Option<u128>,
    pub(crate) last_applescript_status_code: Option<i32>,
    pub(crate) last_applescript_stderr: Option<String>,
    pub(crate) last_enigo_error: Option<String>,
    pub(crate) last_used_fallback: Option<bool>,
    pub(crate) delivery_mode: Option<String>,
    pub(crate) snapshot_taken: Option<bool>,
    pub(crate) restore_attempted: Option<bool>,
    pub(crate) restore_ok: Option<bool>,
    pub(crate) transient_markers_applied: Option<bool>,
    pub(crate) type_error: Option<String>,
    pub(crate) automation_probe: crate::clipboard::injector::AutomationProbeResult,
}

#[derive(Clone, serde::Serialize)]
pub(crate) struct PasteSelfTestResult {
    pub(crate) applescript_result: crate::clipboard::injector::AutomationProbeResult,
    pub(crate) enigo_result: crate::clipboard::injector::EnigoProbeResult,
    pub(crate) selected_path: String,
    pub(crate) final_status: String,
}

fn paste_diag_log_path() -> std::path::PathBuf {
    crate::util::app_data_file("paste_diag.jsonl")
}

pub(crate) fn append_paste_diag_log(entry: &LastPasteAttempt) {
    let path = paste_diag_log_path();
    let now = crate::util::current_unix_timestamp_secs();

    let row = serde_json::json!({
        "timestamp": now,
        "frontmost_before": entry.frontmost_before,
        "frontmost_after": entry.frontmost_after,
        "method": entry.method,
        "error": entry.error,
        "restore_policy": entry.restore_policy,
        "elapsed_ms": entry.elapsed_ms,
        "applescript_status_code": entry.applescript_status_code,
        "applescript_stderr": entry.applescript_stderr,
        "enigo_error": entry.enigo_error,
        "used_fallback": entry.used_fallback,
        "delivery_mode": entry.delivery_mode,
        "snapshot_taken": entry.snapshot_taken,
        "restore_attempted": entry.restore_attempted,
        "restore_ok": entry.restore_ok,
        "transient_markers_applied": entry.transient_markers_applied,
        "type_error": entry.type_error
    });

    let mut lines: Vec<String> = std::fs::read_to_string(&path)
        .ok()
        .map(|s| s.lines().map(|l| l.to_string()).collect())
        .unwrap_or_default();
    lines.push(row.to_string());
    if lines.len() > 500 {
        lines = lines.split_off(lines.len() - 500);
    }
    let content = if lines.is_empty() {
        String::new()
    } else {
        format!("{}\n", lines.join("\n"))
    };
    let _ = std::fs::write(path, content);
}

pub(crate) fn detect_selected_text(app: &AppHandle) -> Result<Option<String>, String> {
    let probe = format!(
        "__koetype_selection_probe_{}__",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    );

    #[cfg(target_os = "macos")]
    {
        use tauri_plugin_clipboard_manager::ClipboardExt;
        let snapshot = crate::clipboard::macos_pasteboard::snapshot_general_pasteboard()
            .map_err(|e| format!("選択判定のクリップボード退避に失敗: {}", e))?;
        crate::clipboard::macos_pasteboard::write_transient_text_for_paste(&probe)
            .map_err(|e| format!("選択判定の初期化に失敗: {}", e))?;

        let copy_result = crate::clipboard::injector::trigger_selection_copy_shortcut()
            .map_err(|e| format!("選択コピーショートカット失敗: {}", e));

        std::thread::sleep(std::time::Duration::from_millis(120));
        let copied = app.clipboard().read_text().ok();

        if let Err(e) = crate::clipboard::macos_pasteboard::restore_general_pasteboard(&snapshot) {
            return Err(format!("選択判定後のクリップボード復元に失敗: {}", e));
        }

        copy_result?;

        let text = copied.unwrap_or_default();
        let trimmed = text.trim().to_string();
        if trimmed.is_empty() || trimmed == probe {
            return Ok(None);
        }
        return Ok(Some(trimmed));
    }

    #[cfg(target_os = "windows")]
    {
        use tauri_plugin_clipboard_manager::ClipboardExt;
        let previous = app.clipboard().read_text().ok();
        app.clipboard()
            .write_text(probe.clone())
            .map_err(|e| format!("選択判定の初期化に失敗: {}", e))?;

        let copy_result = crate::clipboard::injector::trigger_selection_copy_shortcut()
            .map_err(|e| format!("選択コピーショートカット失敗: {}", e));

        std::thread::sleep(std::time::Duration::from_millis(120));
        let copied = app.clipboard().read_text().ok();

        if let Some(prev) = previous {
            let _ = app.clipboard().write_text(prev);
        } else {
            let _ = app.clipboard().clear();
        }

        copy_result?;

        let text = copied.unwrap_or_default();
        let trimmed = text.trim().to_string();
        if trimmed.is_empty() || trimmed == probe {
            return Ok(None);
        }
        return Ok(Some(trimmed));
    }

    #[allow(unreachable_code)]
    Ok(None)
}

/// AppState 上の最新テキスト（メモリ）を優先し、空なら履歴ファイルへフォールバックして解決する。
fn resolve_latest_text(
    app: &AppHandle,
    pick_state: impl FnOnce(&crate::AppState) -> &std::sync::Mutex<Option<String>>,
    fallback: impl FnOnce() -> Option<String>,
) -> Option<String> {
    let state = app.state::<crate::AppState>();
    if let Ok(guard) = pick_state(&state).lock() {
        if let Some(text) = guard.as_ref().map(|t| t.trim().to_string()) {
            if !text.is_empty() {
                return Some(text);
            }
        }
    }
    fallback()
}

pub(crate) fn resolve_latest_os_text(app: &AppHandle) -> Option<String> {
    resolve_latest_text(
        app,
        |s| &s.latest_os_text,
        crate::history::HistoryManager::get_last_dictation_text,
    )
}

/// 補助テキスト（サブ枠に表示するテキスト）の解決。
/// Collaborate モードでは Whisper の結果が補助、それ以外では OS 標準音声入力が補助になる。
pub(crate) fn resolve_latest_aux_text(app: &AppHandle) -> Option<String> {
    if crate::config::get_stt_provider() == crate::config::settings::SttProvider::Collaborate {
        if let Some(text) = crate::history::HistoryManager::get_last_whisper_text() {
            return Some(text);
        }
    }
    resolve_latest_os_text(app)
}

pub(crate) fn resolve_latest_llm_text(app: &AppHandle) -> Option<String> {
    resolve_latest_text(
        app,
        |s| &s.latest_llm_text,
        || crate::history::HistoryManager::get_last_transcription().map(|item| item.text),
    )
}

pub(crate) fn paste_text_preserving_clipboard(app: &AppHandle, text: &str) -> Result<(), String> {
    let started_at = std::time::Instant::now();
    if text.is_empty() {
        return Err("ペースト対象テキストが空です".to_string());
    }
    #[cfg(not(target_os = "macos"))]
    let prev_clipboard = app.clipboard().read_text().ok();
    #[cfg(target_os = "macos")]
    let frontmost_before = crate::clipboard::injector::get_frontmost_app_bundle_id();
    #[cfg(not(target_os = "macos"))]
    let frontmost_before: Option<String> = None;
    let mut last_method: Option<String> = None;
    let mut last_error: Option<String> = None;
    let mut applescript_status_code: Option<i32> = None;
    let mut applescript_stderr: Option<String> = None;
    let mut enigo_error: Option<String> = None;
    let mut used_fallback: Option<bool> = None;
    #[cfg(target_os = "macos")]
    let mut type_error: Option<String> = None;
    #[cfg(not(target_os = "macos"))]
    let type_error: Option<String> = None;
    #[cfg(target_os = "macos")]
    let mut snapshot_taken: Option<bool> = None;
    #[cfg(not(target_os = "macos"))]
    let snapshot_taken: Option<bool> = None;
    #[cfg(target_os = "macos")]
    let mut restore_attempted: Option<bool> = None;
    #[cfg(not(target_os = "macos"))]
    let restore_attempted: Option<bool> = None;
    #[cfg(target_os = "macos")]
    let mut restore_ok: Option<bool> = None;
    #[cfg(not(target_os = "macos"))]
    let restore_ok: Option<bool> = None;
    #[cfg(target_os = "macos")]
    let mut transient_markers_applied: Option<bool> = None;
    #[cfg(not(target_os = "macos"))]
    let transient_markers_applied: Option<bool> = None;

    let delivery_mode = crate::config::settings::get_input_delivery_mode();
    let delivery_mode_label = match delivery_mode {
        crate::config::settings::InputDeliveryMode::Clipboard => "clipboard".to_string(),
        crate::config::settings::InputDeliveryMode::Type => "type".to_string(),
    };

    let restore_policy = match delivery_mode {
        crate::config::settings::InputDeliveryMode::Clipboard => "restore_previous".to_string(),
        crate::config::settings::InputDeliveryMode::Type => "no_clipboard_touch".to_string(),
    };

    macro_rules! save_last_paste_attempt {
        ($frontmost_after:expr) => {{
            let entry = LastPasteAttempt {
                frontmost_before: frontmost_before.clone(),
                frontmost_after: $frontmost_after,
                method: last_method.clone(),
                error: last_error.clone(),
                restore_policy: Some(restore_policy.clone()),
                elapsed_ms: Some(started_at.elapsed().as_millis()),
                applescript_status_code,
                applescript_stderr: applescript_stderr.clone(),
                enigo_error: enigo_error.clone(),
                used_fallback,
                delivery_mode: Some(delivery_mode_label.clone()),
                snapshot_taken,
                restore_attempted,
                restore_ok,
                transient_markers_applied,
                type_error: type_error.clone(),
            };

            if let Ok(mut guard) = app.state::<crate::AppState>().last_paste_attempt.lock() {
                *guard = entry.clone();
            }
            append_paste_diag_log(&entry);
        }};
    }

    #[cfg(target_os = "macos")]
    {
        match delivery_mode {
            crate::config::settings::InputDeliveryMode::Clipboard => {
                let snapshot = crate::clipboard::macos_pasteboard::snapshot_general_pasteboard()
                    .map_err(|e| format!("クリップボードスナップショット失敗: {}", e))?;
                snapshot_taken = Some(true);

                crate::clipboard::macos_pasteboard::write_transient_text_for_paste(text)
                    .map_err(|e| format!("クリップボード一時投入失敗: {}", e))?;
                transient_markers_applied = Some(true);

                use std::sync::mpsc;

                let window = app
                    .get_webview_window("floating")
                    .ok_or_else(|| "floating window not found".to_string())?;

                let (tx, rx) = mpsc::channel::<
                    Result<crate::clipboard::injector::PasteExecutionResult, String>,
                >();
                let text_owned = text.to_string();
                window
                    .run_on_main_thread(move || {
                        let result = crate::clipboard::paste_text_to_active_app(&text_owned)
                            .map_err(|e| e.to_string());
                        let _ = tx.send(result);
                    })
                    .map_err(|e| format!("メインスレッド実行のスケジュールに失敗: {}", e))?;

                match rx.recv_timeout(std::time::Duration::from_secs(3)) {
                    Ok(Ok(result)) => {
                        last_method = Some(result.method);
                        applescript_status_code = result.diagnostics.applescript_status_code;
                        applescript_stderr = result.diagnostics.applescript_stderr;
                        enigo_error = result.diagnostics.enigo_error;
                        used_fallback = Some(result.diagnostics.used_fallback);
                    }
                    Ok(Err(e)) => {
                        last_error = Some(e.clone());
                        let frontmost_after =
                            crate::clipboard::injector::get_frontmost_app_bundle_id();
                        save_last_paste_attempt!(frontmost_after);
                        return Err(e);
                    }
                    Err(_) => {
                        let e = "メインスレッドでのペースト処理がタイムアウトしました".to_string();
                        last_error = Some(e.clone());
                        let frontmost_after =
                            crate::clipboard::injector::get_frontmost_app_bundle_id();
                        save_last_paste_attempt!(frontmost_after);
                        return Err(e);
                    }
                }

                // Cmd+V の貼り付けは対象アプリ側で非同期に処理されるため、
                // 復元が早すぎると貼り付け前にクリップボードが書き戻されてしまう。
                // 重いアプリでも間に合うよう余裕を持って待つ。
                std::thread::sleep(std::time::Duration::from_millis(1500));
                restore_attempted = Some(true);
                match crate::clipboard::macos_pasteboard::restore_general_pasteboard(&snapshot) {
                    Ok(_) => restore_ok = Some(true),
                    Err(e) => {
                        restore_ok = Some(false);
                        warn!("クリップボード復元失敗: {}", e);
                        last_error = Some(match last_error.take() {
                            Some(existing) => format!("{} | restore failed: {}", existing, e),
                            None => format!("restore failed: {}", e),
                        });
                    }
                }
            }
            crate::config::settings::InputDeliveryMode::Type => {
                use std::sync::mpsc;

                let window = app
                    .get_webview_window("floating")
                    .ok_or_else(|| "floating window not found".to_string())?;

                let (tx, rx) = mpsc::channel::<
                    Result<crate::clipboard::injector::TypeExecutionResult, String>,
                >();
                let text_owned = text.to_string();
                window
                    .run_on_main_thread(move || {
                        let result =
                            crate::clipboard::injector::type_text_to_active_app(&text_owned)
                                .map_err(|e| e.to_string());
                        let _ = tx.send(result);
                    })
                    .map_err(|e| format!("メインスレッド実行のスケジュールに失敗: {}", e))?;

                match rx.recv_timeout(std::time::Duration::from_secs(8)) {
                    Ok(Ok(result)) => {
                        last_method = Some(result.method);
                        type_error = result.diagnostics.enigo_error;
                    }
                    Ok(Err(e)) => {
                        last_error = Some(e.clone());
                        type_error = Some(e.clone());
                        let frontmost_after =
                            crate::clipboard::injector::get_frontmost_app_bundle_id();
                        save_last_paste_attempt!(frontmost_after);
                        return Err(e);
                    }
                    Err(_) => {
                        let e = "メインスレッドでの直接入力処理がタイムアウトしました".to_string();
                        last_error = Some(e.clone());
                        type_error = Some(e.clone());
                        let frontmost_after =
                            crate::clipboard::injector::get_frontmost_app_bundle_id();
                        save_last_paste_attempt!(frontmost_after);
                        return Err(e);
                    }
                }
            }
        }
    }

    #[cfg(not(target_os = "macos"))]
    {
        use tauri_plugin_clipboard_manager::ClipboardExt;
        match crate::clipboard::paste_text_to_active_app(text) {
            Ok(result) => {
                last_method = Some(result.method);
                applescript_status_code = result.diagnostics.applescript_status_code;
                applescript_stderr = result.diagnostics.applescript_stderr;
                enigo_error = result.diagnostics.enigo_error;
                used_fallback = Some(result.diagnostics.used_fallback);
            }
            Err(e) => {
                let e = e.to_string();
                last_error = Some(e.clone());
                save_last_paste_attempt!(None);
                return Err(e);
            }
        }
        // Ctrl+V の処理完了を待ってから復元する（早すぎると貼り付け前に書き戻される）
        std::thread::sleep(std::time::Duration::from_millis(200));
        match prev_clipboard {
            Some(prev_text) => {
                if let Err(e) = app.clipboard().write_text(prev_text) {
                    warn!("クリップボード復元失敗: {}", e);
                }
            }
            None => {
                if let Err(e) = app.clipboard().clear() {
                    warn!("クリップボードクリア失敗: {}", e);
                }
            }
        }
    }
    #[cfg(target_os = "macos")]
    let frontmost_after = crate::clipboard::injector::get_frontmost_app_bundle_id();
    #[cfg(not(target_os = "macos"))]
    let frontmost_after: Option<String> = None;
    save_last_paste_attempt!(frontmost_after);
    Ok(())
}

#[tauri::command]
pub(crate) fn diagnose_paste_runtime(app: tauri::AppHandle) -> Result<PasteRuntimeDiag, String> {
    let executable_path = std::env::current_exe()
        .map(|p| p.display().to_string())
        .unwrap_or_else(|_| "unknown".to_string());
    let bundle_identifier = app.config().identifier.clone();
    let codesign_cdhash = std::process::Command::new("codesign")
        .arg("-dv")
        .arg("--verbose=4")
        .arg(&executable_path)
        .output()
        .ok()
        .and_then(|o| {
            let text = String::from_utf8_lossy(&o.stderr);
            text.lines()
                .find_map(|line| line.strip_prefix("CDHash=").map(|s| s.trim().to_string()))
        });
    let last = app
        .state::<crate::AppState>()
        .last_paste_attempt
        .lock()
        .ok()
        .map(|g| g.clone())
        .unwrap_or_default();

    Ok(PasteRuntimeDiag {
        executable_path,
        bundle_identifier,
        codesign_cdhash,
        ax_trusted: crate::clipboard::injector::is_macos_accessibility_trusted(),
        frontmost_app_bundle_id: crate::clipboard::injector::get_frontmost_app_bundle_id(),
        frontmost_before_paste: last.frontmost_before,
        frontmost_after_paste: last.frontmost_after,
        last_paste_method: last.method,
        last_paste_error: last.error,
        last_restore_policy: last.restore_policy,
        last_paste_elapsed_ms: last.elapsed_ms,
        last_applescript_status_code: last.applescript_status_code,
        last_applescript_stderr: last.applescript_stderr,
        last_enigo_error: last.enigo_error,
        last_used_fallback: last.used_fallback,
        delivery_mode: last.delivery_mode,
        snapshot_taken: last.snapshot_taken,
        restore_attempted: last.restore_attempted,
        restore_ok: last.restore_ok,
        transient_markers_applied: last.transient_markers_applied,
        type_error: last.type_error,
        automation_probe: crate::clipboard::injector::run_applescript_probe(),
    })
}

#[tauri::command]
pub(crate) fn run_paste_self_test(_app: tauri::AppHandle) -> Result<PasteSelfTestResult, String> {
    let applescript_result = crate::clipboard::injector::run_applescript_probe();

    #[cfg(target_os = "macos")]
    let enigo_result = {
        use std::sync::mpsc;
        let window = _app
            .get_webview_window("floating")
            .ok_or_else(|| "floating window not found".to_string())?;
        let (tx, rx) = mpsc::channel::<crate::clipboard::injector::EnigoProbeResult>();
        window
            .run_on_main_thread(move || {
                let result = crate::clipboard::injector::run_enigo_probe();
                let _ = tx.send(result);
            })
            .map_err(|e| format!("メインスレッド実行のスケジュールに失敗: {}", e))?;
        rx.recv_timeout(std::time::Duration::from_secs(3))
            .map_err(|_| "self-test timeout".to_string())?
    };

    #[cfg(not(target_os = "macos"))]
    let enigo_result = crate::clipboard::injector::run_enigo_probe();

    let ax = crate::clipboard::injector::is_macos_accessibility_trusted();
    let (selected_path, final_status) = if ax {
        if enigo_result.ok {
            ("enigo".to_string(), "success".to_string())
        } else if applescript_result.ok {
            ("applescript".to_string(), "indeterminate".to_string())
        } else {
            ("none".to_string(), "permission_denied".to_string())
        }
    } else if applescript_result.ok {
        ("applescript".to_string(), "indeterminate".to_string())
    } else {
        ("none".to_string(), "permission_denied".to_string())
    };

    Ok(PasteSelfTestResult {
        applescript_result,
        enigo_result,
        selected_path,
        final_status,
    })
}
