use std::path::Path;
#[cfg(target_os = "macos")]
use std::process::Command;
#[cfg(target_os = "windows")]
use std::process::Command;
#[cfg(target_os = "macos")]
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter, Manager, State};
use tauri_plugin_clipboard_manager::ClipboardExt;
use tauri_plugin_global_shortcut::GlobalShortcutExt;
use tracing::warn;

use crate::config::settings::{ShortcutSettings, SttProvider};
use crate::AppState;

pub(crate) const WHISPER_RUNTIME_MISSING_GUIDANCE_MESSAGE: &str =
    "設定のトラブルシューティングからWhisperをインストールしてください";

// SpectrumUiSettings: マイク感度・波形スケール設定のUI通知用ペイロード
#[derive(Clone, serde::Serialize)]
struct SpectrumUiSettingsPayload {
    mic_sensitivity: f64,
    wave_motion_scale: f64,
}

#[derive(Clone, serde::Serialize)]
pub(crate) struct OnboardingStatusPayload {
    is_macos: bool,
    ack_unnotarized: bool,
    completed_at: Option<i64>,
    ax_trusted: bool,
    permission_probe_ok: Option<bool>,
    whisper_cli_path: Option<String>,
    whisper_model_path: Option<String>,
    whisper_bundled_cli: bool,
    whisper_bundled_model: bool,
    whisper_ready: bool,
    all_ready: bool,
}

#[derive(Clone, serde::Serialize)]
pub(crate) struct PermissionProbePayload {
    success: bool,
    detail: String,
}

fn get_onboarding_status_payload() -> OnboardingStatusPayload {
    let whisper = crate::transcription::whisper_local::inspect_whisper_runtime_status_auto();
    #[cfg(target_os = "macos")]
    let ax_trusted = crate::clipboard::injector::is_macos_accessibility_trusted();
    #[cfg(not(target_os = "macos"))]
    let ax_trusted = false;

    let permission_probe_ok = crate::config::settings::get_onboarding_permission_probe_ok();
    let whisper_ready = whisper.cli_path.is_some() && whisper.model_path.is_some();
    let all_ready = ax_trusted && whisper_ready && permission_probe_ok.unwrap_or(false);

    OnboardingStatusPayload {
        is_macos: cfg!(target_os = "macos"),
        ack_unnotarized: crate::config::settings::get_onboarding_ack_unnotarized(),
        completed_at: crate::config::settings::get_onboarding_completed_at(),
        ax_trusted,
        permission_probe_ok,
        whisper_cli_path: whisper.cli_path,
        whisper_model_path: whisper.model_path,
        whisper_bundled_cli: whisper.cli_bundled,
        whisper_bundled_model: whisper.model_bundled,
        whisper_ready,
        all_ready,
    }
}

fn get_spectrum_ui_settings_payload() -> SpectrumUiSettingsPayload {
    SpectrumUiSettingsPayload {
        mic_sensitivity: crate::config::settings::get_mic_sensitivity(),
        wave_motion_scale: crate::config::settings::get_wave_motion_scale(),
    }
}

// APIキー設定コマンド
#[tauri::command]
pub(crate) fn set_api_key(api_key: String) -> Result<String, String> {
    crate::config::set_api_key(&api_key)
        .map(|_| "APIキーを保存しました".to_string())
        .map_err(|e| e.to_string())
}

// APIキー取得コマンド（実際のキーを返す）
#[tauri::command]
pub(crate) fn get_api_key() -> Result<String, String> {
    crate::config::get_api_key().map_err(|e| e.to_string())
}

// クリップボードにコピーのみ
#[tauri::command]
pub(crate) fn copy_text(app: AppHandle, text: String) -> Result<String, String> {
    app.clipboard()
        .write_text(text)
        .map_err(|e| e.to_string())?;
    Ok("copied".to_string())
}

#[tauri::command]
#[cfg(target_os = "windows")]
pub(crate) fn probe_winrt_speech_languages() -> Result<serde_json::Value, String> {
    use windows::Media::SpeechRecognition::SpeechRecognizer;

    let langs = SpeechRecognizer::SupportedTopicLanguages()
        .map_err(|e| format!("SupportedTopicLanguages取得に失敗: {}", e))?;
    let size = langs
        .Size()
        .map_err(|e| format!("言語リストサイズ取得に失敗: {}", e))?;

    let mut items: Vec<String> = Vec::new();
    for i in 0..size {
        let lang = langs
            .GetAt(i)
            .map_err(|e| format!("言語取得に失敗: {}", e))?;
        let tag = lang
            .LanguageTag()
            .map_err(|e| format!("LanguageTag取得に失敗: {}", e))?;
        items.push(tag.to_string());
    }

    items.sort();
    items.dedup();

    let has_ja = items.iter().any(|v| v.eq_ignore_ascii_case("ja-JP"));
    let has_en = items.iter().any(|v| v.eq_ignore_ascii_case("en-US"));

    Ok(serde_json::json!({
        "supported_topic_languages": items,
        "has_ja_JP": has_ja,
        "has_en_US": has_en
    }))
}

#[tauri::command]
#[cfg(not(target_os = "windows"))]
pub(crate) fn probe_winrt_speech_languages() -> Result<serde_json::Value, String> {
    Err("このコマンドはWindows専用です".to_string())
}

#[tauri::command]
#[cfg(target_os = "windows")]
pub(crate) fn probe_sapi_recognizers() -> Result<serde_json::Value, String> {
    use winreg::enums::HKEY_LOCAL_MACHINE;
    use winreg::RegKey;

    fn parse_languages(raw: &str) -> Vec<u32> {
        raw.split(&[';', ','][..])
            .filter_map(|part| {
                let trimmed = part.trim();
                if trimmed.is_empty() {
                    return None;
                }
                if let Ok(v) = u32::from_str_radix(trimmed.trim_start_matches("0x"), 16) {
                    return Some(v);
                }
                trimmed.parse::<u32>().ok()
            })
            .collect()
    }

    let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
    let paths = vec![
        r"SOFTWARE\Microsoft\Speech\Recognizers\Tokens",
        r"SOFTWARE\Microsoft\Speech_OneCore\Recognizers\Tokens",
    ];

    let mut tokens: Vec<serde_json::Value> = Vec::new();
    let mut all_langs: Vec<u32> = Vec::new();

    for path in paths {
        let key = match hklm.open_subkey(path) {
            Ok(k) => k,
            Err(_) => continue,
        };
        for name in key.enum_keys().flatten() {
            if let Ok(token_key) = key.open_subkey(&name) {
                let attrs = token_key.open_subkey("Attributes").ok();
                let lang_raw = attrs
                    .as_ref()
                    .and_then(|k| k.get_value::<String, _>("Language").ok())
                    .unwrap_or_default();
                let langs = parse_languages(&lang_raw);
                all_langs.extend(langs.iter().copied());

                let description = token_key
                    .get_value::<String, _>("(Default)")
                    .ok()
                    .filter(|s| !s.trim().is_empty());
                let clsid = token_key.get_value::<String, _>("CLSID").ok();
                let vendor = attrs
                    .as_ref()
                    .and_then(|k| k.get_value::<String, _>("Vendor").ok());
                let name_attr = attrs
                    .as_ref()
                    .and_then(|k| k.get_value::<String, _>("Name").ok());

                tokens.push(serde_json::json!({
                    "token_key": name,
                    "description": description,
                    "name": name_attr,
                    "vendor": vendor,
                    "language_raw": lang_raw,
                    "languages": langs,
                    "clsid": clsid,
                }));
            }
        }
    }

    all_langs.sort();
    all_langs.dedup();

    let has_ja = all_langs.iter().any(|v| *v == 0x0411);
    let has_en = all_langs.iter().any(|v| *v == 0x0409);

    Ok(serde_json::json!({
        "recognizers": tokens,
        "languages": all_langs,
        "has_ja_JP": has_ja,
        "has_en_US": has_en,
    }))
}

#[tauri::command]
#[cfg(not(target_os = "windows"))]
pub(crate) fn probe_sapi_recognizers() -> Result<serde_json::Value, String> {
    Err("このコマンドはWindows専用です".to_string())
}

/// フロントエンドからのデバッグログを受け取る
#[tauri::command]
pub(crate) fn log_frontend_event(message: String) {
    let lowered = message.to_lowercase();
    let is_error_like = message.contains("エラー")
        || message.contains("失敗")
        || lowered.contains("error")
        || lowered.contains("failed");
    if is_error_like {
        warn!("フロントエンド: {}", message);
        let _ = crate::error_log::ErrorLogManager::add_error(
            crate::error_log::ErrorType::UnknownError,
            &message,
            Some("frontend_event"),
        );
    } else {
        tracing::debug!("フロントエンド: {}", message);
    }
}

// 使用モデル取得コマンド
#[tauri::command]
pub(crate) fn get_model() -> String {
    crate::config::get_model()
}

#[tauri::command]
pub(crate) fn get_ai_model() -> String {
    crate::config::get_ai_model()
}

// 使用モデル設定コマンド
#[tauri::command]
pub(crate) fn set_model(model: String) -> Result<String, String> {
    crate::config::set_model(&model)
        .map(|_| format!("使用モデルを {} に設定しました", model))
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub(crate) fn set_ai_model(model: String) -> Result<String, String> {
    crate::config::set_ai_model(&model)
        .map(|_| format!("AI処理用モデルを {} に設定しました", model))
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub(crate) fn get_whisper_model_path() -> Option<String> {
    crate::config::settings::get_whisper_model_path()
}

#[tauri::command]
pub(crate) fn set_whisper_model_path(model_path: Option<String>) -> Result<String, String> {
    if let Some(path) = model_path
        .as_ref()
        .map(|v| v.trim())
        .filter(|v| !v.is_empty())
    {
        let is_bin = Path::new(path)
            .extension()
            .and_then(|ext| ext.to_str())
            .map(|ext| ext.eq_ignore_ascii_case("bin"))
            .unwrap_or(false);
        if !is_bin {
            return Err(format!(
                "Whisperモデルは .bin ファイルを指定してください: {}",
                path
            ));
        }
    }

    crate::config::settings::set_whisper_model_path(model_path)
        .map(|_| "Whisperモデルパスを設定しました".to_string())
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub(crate) fn pick_whisper_model_path() -> Option<String> {
    rfd::FileDialog::new()
        .add_filter("Whisper model", &["bin"])
        .pick_file()
        .map(|path| path.to_string_lossy().to_string())
}

#[tauri::command]
pub(crate) fn get_whisper_model_paths() -> Vec<String> {
    crate::config::settings::get_whisper_model_paths()
}

#[tauri::command]
pub(crate) fn set_whisper_model_paths(paths: Vec<String>) -> Result<String, String> {
    crate::config::settings::set_whisper_model_paths(paths)
        .map(|_| "Whisperモデルパス一覧を更新しました".to_string())
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub(crate) async fn install_whisper_model(app: AppHandle) -> Result<String, String> {
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<
        crate::transcription::whisper_local::WhisperInstallProgress,
    >();

    let task = tokio::task::spawn_blocking(move || {
        let mut emit_progress =
            |progress: crate::transcription::whisper_local::WhisperInstallProgress| {
                let _ = tx.send(progress);
            };
        let default_model = crate::transcription::whisper_local::default_model_name_for_platform();
        crate::transcription::whisper_local::install_whisper_model_with_progress(
            default_model,
            &mut emit_progress,
        )
    });

    while let Some(progress) = rx.recv().await {
        let _ = app.emit("whisper-install-progress", progress);
    }

    match task
        .await
        .map_err(|e| format!("インストール処理に失敗: {}", e))?
    {
        Ok(model_path) => {
            crate::config::settings::set_whisper_model_path(Some(model_path.clone()))
                .map_err(|e| e.to_string())?;
            if let Err(e) = hide_floating_ui(app.clone()) {
                warn!(
                    "Whisperインストール完了後のフローティング非表示に失敗: {}",
                    e
                );
            }
            Ok(format!("Whisperモデルを配置しました: {}", model_path))
        }
        Err(error) => {
            let _ = app.emit(
                "whisper-install-progress",
                crate::transcription::whisper_local::WhisperInstallProgress {
                    phase: "failed".to_string(),
                    downloaded_bytes: 0,
                    total_bytes: None,
                    percent: None,
                    message: error.clone(),
                },
            );
            Err(error)
        }
    }
}

#[tauri::command]
pub(crate) fn get_stt_provider() -> String {
    match crate::config::get_stt_provider() {
        SttProvider::Gemini => "gemini".to_string(),
        SttProvider::Hybrid => "hybrid".to_string(),
        SttProvider::Collaborate => "collaborate".to_string(),
        SttProvider::Whisper => "whisper".to_string(),
    }
}

#[tauri::command]
pub(crate) fn set_stt_provider(app: AppHandle, provider: String) -> Result<String, String> {
    let parsed = match provider.as_str() {
        "gemini" => SttProvider::Gemini,
        "hybrid" => SttProvider::Hybrid,
        "collaborate" => SttProvider::Collaborate,
        "whisper" => SttProvider::Whisper,
        _ => return Err(format!("未対応の文字起こしプロバイダです: {}", provider)),
    };

    crate::config::set_stt_provider(parsed.clone()).map_err(|e| e.to_string())?;

    if parsed == SttProvider::Gemini {
        if let Err(e) = hide_floating_ui(app) {
            warn!("Gemini切替時のフローティング非表示に失敗: {}", e);
        }
    }

    Ok(format!(
        "文字起こしプロバイダを {} に設定しました",
        provider
    ))
}

#[tauri::command]
pub(crate) fn hide_floating_ui(app: tauri::AppHandle) -> Result<String, String> {
    if let Some(window) = app.get_webview_window("floating") {
        let _ = window.hide();
    }
    #[cfg(target_os = "macos")]
    {
        crate::overlay::overlay_send_json(
            &app,
            serde_json::json!({
                "type": "hide"
            }),
        );
        crate::overlay::shutdown_overlay(&app);
    }
    Ok("フローティングUIを非表示にしました".to_string())
}

#[tauri::command]
#[cfg(target_os = "macos")]
pub(crate) fn open_terminal_install_whisper() -> Result<String, String> {
    use std::io::Write;
    use std::os::unix::fs::PermissionsExt;
    let log_and_err = |message: String| -> Result<String, String> {
        let full = format!("Whisperインストール起動失敗: {}", message);
        let _ = crate::error_log::ErrorLogManager::add_error(
            crate::error_log::ErrorType::UnknownError,
            &full,
            Some("whisper_install"),
        );
        Err(full)
    };

    const MODEL_FILENAME: &str = "ggml-large-v3-turbo-q5_0.bin";
    const MODEL_URL: &str =
        "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-large-v3-turbo-q5_0.bin";

    let app_home = crate::config::settings::get_app_home_dir();
    let model_dir = app_home.join("models/whisper");
    let model_path = model_dir.join(MODEL_FILENAME);
    let tmp_path = model_dir.join(format!("{}.tmp", MODEL_FILENAME));

    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    // 一時シェルスクリプトを生成し、Terminal.app へ AppleScript 経由で
    // /bin/bash --noprofile --norc 実行を指示する。
    let script_path = std::env::temp_dir().join(format!("kw_{}.sh", ts));
    let script = format!(
        concat!(
            "#!/bin/bash\n",
            // 成功・失敗どちらでも EXIT 時にウィンドウを保持する\n",
            "trap 'echo \"\"; read -rp \"[Enterキーで閉じる] \" || true' EXIT\n",
            "set -e\n",
            "unset AUTO_TMUX\n",
            "export AUTO_TMUX=\n",
            "unset TMUX\n",
            "export TMUX=\n",
            "echo 'Whisperモデルをダウンロードします...'\n",
            "echo '保存先: {model_path}'\n",
            "echo ''\n",
            "mkdir -p '{model_dir}'\n",
            "curl -L --progress-bar '{model_url}' -o '{tmp_path}'\n",
            "mv '{tmp_path}' '{model_path}'\n",
            "echo ''\n",
            "echo 'インストール完了: {model_path}'\n",
            "echo 'KoeType の設定画面で「状態を確認」を押してください。'\n",
        ),
        model_dir = model_dir.to_string_lossy(),
        model_path = model_path.to_string_lossy(),
        model_url = MODEL_URL,
        tmp_path = tmp_path.to_string_lossy(),
    );

    {
        let mut file = std::fs::File::create(&script_path)
            .map_err(|e| format!("インストールスクリプト作成に失敗: {}", e))
            .or_else(log_and_err)?;
        file.write_all(script.as_bytes())
            .map_err(|e| format!("スクリプト書き込みに失敗: {}", e))
            .or_else(log_and_err)?;
    }
    std::fs::set_permissions(&script_path, std::fs::Permissions::from_mode(0o755))
        .map_err(|e| format!("スクリプト権限設定に失敗: {}", e))
        .or_else(log_and_err)?;

    let applescript = format!(
        r#"set installScript to quoted form of POSIX path of "{path}"
tell application "Terminal"
  activate
  do script "unset AUTO_TMUX; export AUTO_TMUX=; unset TMUX; export TMUX=; exec /bin/bash --noprofile --norc " & installScript
end tell"#,
        path = script_path.to_string_lossy()
    );
    let output = Command::new("osascript")
        .arg("-e")
        .arg(applescript)
        .output()
        .map_err(|e| format!("Terminal 起動に失敗しました: {}", e))
        .or_else(log_and_err)?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return log_and_err(format!(
            "Terminal 起動コマンド(osascript)が失敗しました: {}",
            stderr
        ));
    }

    Ok(model_path.to_string_lossy().to_string())
}

#[tauri::command]
#[cfg(target_os = "windows")]
pub(crate) fn open_terminal_install_whisper() -> Result<String, String> {
    use std::time::{SystemTime, UNIX_EPOCH};
    let log_err = |message: String| -> String {
        let full = format!("Whisperインストール起動失敗: {}", message);
        let _ = crate::error_log::ErrorLogManager::add_error(
            crate::error_log::ErrorType::UnknownError,
            &full,
            Some("whisper_install"),
        );
        full
    };

    const MODEL_NAME: &str = "base";
    const MODEL_VARIANT: &str = "q5_1";
    const MODEL_FILENAME: &str = "ggml-base-q5_1.bin";
    const BACKEND: &str = "cpu";

    let app_home = crate::config::settings::get_app_home_dir();
    let model_dir = app_home.join("models").join("whisper");
    let model_path = model_dir.join(MODEL_FILENAME);
    let configured_model_path = model_path.to_string_lossy().to_string();

    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let script_path = std::env::temp_dir().join(format!("koetype_setup_whisper_{}.ps1", ts));
    let script = include_str!("../../../scripts/windows/setup-whisper.ps1");
    std::fs::write(&script_path, script.as_bytes())
        .map_err(|e| log_err(format!("インストールスクリプト作成に失敗: {}", e)))?;

    Command::new("cmd.exe")
        .args([
            "/c",
            "start",
            "KoeType Whisper Setup",
            "powershell.exe",
            "-NoProfile",
            "-ExecutionPolicy",
            "Bypass",
            "-File",
            &script_path.to_string_lossy().to_string(),
            "-Model",
            MODEL_NAME,
            "-Variant",
            MODEL_VARIANT,
            "-Backend",
            BACKEND,
            "-NonInteractive",
        ])
        .spawn()
        .map_err(|e| log_err(format!("PowerShell を開けませんでした: {}", e)))?;

    Ok(configured_model_path)
}

#[tauri::command]
#[cfg(not(any(target_os = "macos", target_os = "windows")))]
pub(crate) fn open_terminal_install_whisper() -> Result<String, String> {
    let message = "このコマンドはmacOS/Windows専用です".to_string();
    let full = format!("Whisperインストール起動失敗: {}", message);
    let _ = crate::error_log::ErrorLogManager::add_error(
        crate::error_log::ErrorType::UnknownError,
        &full,
        Some("whisper_install"),
    );
    Err(full)
}

#[tauri::command]
#[cfg(target_os = "macos")]
pub(crate) fn force_kill_floating_overlay() -> Result<String, String> {
    let output = Command::new("pkill")
        .args(["-f", "floating-overlay"])
        .output()
        .map_err(|e| format!("pkill 実行失敗: {}", e))?;
    Ok(format!(
        "floating-overlay プロセスへの強制終了を実行しました (exit: {})",
        output.status.code().unwrap_or(-1)
    ))
}

#[tauri::command]
#[cfg(not(target_os = "macos"))]
pub(crate) fn force_kill_floating_overlay() -> Result<String, String> {
    Err("このコマンドはmacOS専用です".to_string())
}

fn open_settings_tab(app: tauri::AppHandle, tab: &str) -> Result<(), String> {
    if let Some(window) = app.get_webview_window("settings") {
        let _ = window.show();
        let _ = window.set_focus();
        let _ = window.emit("switch-tab", tab);
        Ok(())
    } else {
        let window = tauri::WebviewWindowBuilder::new(
            &app,
            "settings",
            tauri::WebviewUrl::App(format!("index.html#{}", tab).into()),
        )
        .title("KoeType Settings")
        .inner_size(1200.0, 900.0)
        .min_inner_size(1200.0, 700.0)
        .resizable(true)
        .fullscreen(false)
        .decorations(true)
        .transparent(false)
        .always_on_top(false)
        .build()
        .map_err(|e| format!("設定ウィンドウ作成エラー: {}", e))?;

        let _ = window.show();
        let _ = window.set_focus();
        Ok(())
    }
}

pub(crate) fn open_settings_tab_internal(app: tauri::AppHandle, tab: &str) -> Result<(), String> {
    open_settings_tab(app, tab)
}

// 設定ウィンドウを開くコマンド
#[tauri::command]
pub(crate) fn open_settings_window(
    app: tauri::AppHandle,
    _state: State<AppState>,
) -> Result<(), String> {
    open_settings_tab(app, "settings")
}

#[cfg(target_os = "macos")]
fn run_macos_permission_probe(app: &AppHandle) -> Result<PermissionProbePayload, String> {
    fn wait_with_timeout(
        child: &mut std::process::Child,
        timeout: Duration,
        timeout_label: &str,
    ) -> Result<std::process::ExitStatus, String> {
        let started = Instant::now();
        loop {
            if let Some(status) = child
                .try_wait()
                .map_err(|e| format!("permission probe wait失敗: {}", e))?
            {
                return Ok(status);
            }
            if started.elapsed() >= timeout {
                let _ = child.kill();
                return Err(timeout_label.to_string());
            }
            std::thread::sleep(Duration::from_millis(200));
        }
    }

    let (candidates, sidecar_path) =
        crate::overlay::find_macos_sidecar_path(app, "speech-recognizer");
    let Some(sidecar_path) = sidecar_path else {
        return Err(format!(
            "speech-recognizer sidecar が見つかりません。候補: {}",
            candidates
                .iter()
                .map(|p| p.display().to_string())
                .collect::<Vec<String>>()
                .join(", ")
        ));
    };

    let locale = match crate::config::settings::get_language().as_str() {
        "en" => "en-US",
        _ => "ja-JP",
    };
    let dummy_output = std::env::temp_dir().join("koetype_permission_probe.wav");
    let sidecar_app_path = sidecar_path
        .parent()
        .and_then(|p| p.parent())
        .and_then(|p| p.parent())
        .map(|p| p.to_path_buf())
        .ok_or_else(|| {
            format!(
                "sidecar app パスの解決に失敗しました: {}",
                sidecar_path.display()
            )
        })?;

    // 1) 権限要求は sidecar .app 主体で実行（要求元が KoeType ではなく sidecar になる）
    let mut open_child = Command::new("open")
        .arg("-n")
        .arg("-W")
        .arg(&sidecar_app_path)
        .arg("--args")
        .arg(dummy_output.as_os_str())
        .arg(locale)
        .arg("--speech")
        .arg("--permissions-only")
        .spawn()
        .map_err(|e| format!("permission probe 起動失敗(open): {}", e))?;
    let open_timeout = Duration::from_secs(180);
    match wait_with_timeout(
        &mut open_child,
        open_timeout,
        "権限確認がタイムアウトしました。システム設定から再確認してください。",
    ) {
        Ok(status) => {
            let success = status.success();
            let detail = if success {
                "KoeType Speech Recognizer のマイク/音声認識の権限確認に成功しました".to_string()
            } else {
                format!("open 終了コード: {:?}", status.code())
            };
            Ok(PermissionProbePayload { success, detail })
        }
        Err(detail) => Ok(PermissionProbePayload {
            success: false,
            detail,
        }),
    }
}

#[tauri::command]
pub(crate) fn get_onboarding_status() -> OnboardingStatusPayload {
    get_onboarding_status_payload()
}

#[tauri::command]
pub(crate) fn set_onboarding_ack_unnotarized(ack: bool) -> Result<OnboardingStatusPayload, String> {
    crate::config::settings::set_onboarding_ack_unnotarized(ack).map_err(|e| e.to_string())?;
    Ok(get_onboarding_status_payload())
}

#[tauri::command]
pub(crate) fn mark_onboarding_completed() -> Result<OnboardingStatusPayload, String> {
    crate::config::settings::set_onboarding_completed_now().map_err(|e| e.to_string())?;
    Ok(get_onboarding_status_payload())
}

#[tauri::command]
pub(crate) fn run_permissions_probe(
    app: tauri::AppHandle,
) -> Result<PermissionProbePayload, String> {
    #[cfg(target_os = "macos")]
    {
        let had_ax = crate::clipboard::injector::is_macos_accessibility_trusted();
        if !had_ax {
            let _ = crate::clipboard::injector::request_macos_accessibility_prompt();
        }
        let result = run_macos_permission_probe(&app)?;
        crate::config::settings::set_onboarding_permission_probe_ok(result.success)
            .map_err(|e| e.to_string())?;
        return Ok(result);
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = app;
        Err("このコマンドはmacOS専用です".to_string())
    }
}

#[tauri::command]
pub(crate) fn open_macos_privacy_page(_section: String) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        let fragment = match _section.as_str() {
            "accessibility" => "Privacy_Accessibility",
            "microphone" => "Privacy_Microphone",
            "speech" => "Privacy_SpeechRecognition",
            other => return Err(format!("未対応のプライバシー項目です: {}", other)),
        };
        let url = format!(
            "x-apple.systempreferences:com.apple.preference.security?{}",
            fragment
        );
        Command::new("open")
            .arg(url)
            .spawn()
            .map_err(|e| format!("システム設定を開けませんでした: {}", e))?;
        return Ok(());
    }
    #[cfg(not(target_os = "macos"))]
    {
        Err("このコマンドはmacOS専用です".to_string())
    }
}

#[tauri::command]
pub(crate) fn check_for_updates() -> Result<String, String> {
    let url = std::env::var("KOETYPE_RELEASES_URL")
        .ok()
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| "https://github.com/yasu-888/koetype/releases/latest".to_string());
    #[cfg(target_os = "macos")]
    {
        Command::new("open")
            .arg(&url)
            .spawn()
            .map_err(|e| format!("Releaseページを開けませんでした: {}", e))?;
    }
    #[cfg(target_os = "windows")]
    {
        Command::new("cmd")
            .args(["/C", "start", "", &url])
            .spawn()
            .map_err(|e| format!("Releaseページを開けませんでした: {}", e))?;
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        return Err("このプラットフォームでは未対応です".to_string());
    }
    Ok(url)
}

// ウィンドウが実際に画面に表示されているか確認するコマンド（Occlusion State）
#[tauri::command]
pub(crate) fn is_window_occluded(_window: tauri::Window) -> bool {
    #[cfg(target_os = "macos")]
    {
        return is_window_occluded_macos(&_window);
    }
    #[cfg(not(target_os = "macos"))]
    {
        false
    }
}

#[cfg(target_os = "macos")]
#[allow(unexpected_cfgs)]
fn is_window_occluded_macos(window: &tauri::Window) -> bool {
    use objc::runtime::Object;
    use objc::{msg_send, sel, sel_impl};

    let Ok(raw_window) = window.ns_window() else {
        return false;
    };
    let ns_window = raw_window as *mut Object;
    if !ns_window.is_null() {
        unsafe {
            let occlusion_state: u64 = msg_send![ns_window, occlusionState];
            // NSWindowOcclusionStateVisible = 1 << 1
            return (occlusion_state & 2) == 0;
        }
    }
    false
}

#[tauri::command]
pub(crate) fn get_feedback_settings() -> crate::FeedbackSettings {
    crate::FeedbackSettings {
        notification_mode: crate::config::settings::get_notification_mode(),
        sound_mode: crate::config::settings::get_sound_mode(),
    }
}

#[tauri::command]
pub(crate) fn set_sound_setting(
    app: tauri::AppHandle,
    mode: crate::config::settings::FeedbackMode,
) -> Result<(), String> {
    crate::config::settings::set_sound_mode(mode.clone()).map_err(|e| e.to_string())?;

    app.emit("settings-updated", get_feedback_settings())
        .map_err(|e| e.to_string())?;

    Ok(())
}

#[tauri::command]
pub(crate) fn get_input_delivery_mode() -> crate::config::settings::InputDeliveryMode {
    crate::config::settings::get_input_delivery_mode()
}

#[tauri::command]
pub(crate) fn set_input_delivery_mode(
    mode: crate::config::settings::InputDeliveryMode,
) -> Result<(), String> {
    crate::config::settings::set_input_delivery_mode(mode).map_err(|e| e.to_string())
}

#[tauri::command]
pub(crate) fn get_mic_sensitivity() -> f64 {
    crate::config::settings::get_mic_sensitivity()
}

#[tauri::command]
pub(crate) fn set_mic_sensitivity(app: tauri::AppHandle, value: f64) -> Result<(), String> {
    crate::config::settings::set_mic_sensitivity(value).map_err(|e| e.to_string())?;
    app.emit(
        "spectrum-ui-settings-updated",
        get_spectrum_ui_settings_payload(),
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub(crate) fn get_wave_motion_scale() -> f64 {
    crate::config::settings::get_wave_motion_scale()
}

#[tauri::command]
pub(crate) fn set_wave_motion_scale(app: tauri::AppHandle, value: f64) -> Result<(), String> {
    crate::config::settings::set_wave_motion_scale(value).map_err(|e| e.to_string())?;
    app.emit(
        "spectrum-ui-settings-updated",
        get_spectrum_ui_settings_payload(),
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub(crate) fn cancel_transcription(
    app: tauri::AppHandle,
    state: State<AppState>,
) -> Result<(), String> {
    if let Ok(mut seq) = state.transcription_cancel_seq.lock() {
        *seq = seq.saturating_add(1);
    }
    let _ = app.emit("transcription-cancelled", ());
    crate::overlay::overlay_send_json(
        &app,
        serde_json::json!({
            "type": "transcription-cancelled"
        }),
    );
    Ok(())
}

// Gemini API接続テストコマンド
#[tauri::command]
pub(crate) async fn test_gemini_connection(
    api_key: String,
    model: String,
) -> Result<String, String> {
    crate::transcription::test_connection(&api_key, &model)
        .await
        .map_err(|e| e.to_string())
}

// マイクデバイス一覧取得コマンド
#[tauri::command]
pub(crate) fn list_audio_devices() -> Vec<crate::audio::recorder::AudioDevice> {
    crate::audio::recorder::list_input_devices()
}

// マイクデバイス設定コマンド
#[tauri::command]
pub(crate) fn set_audio_device(
    state: State<AppState>,
    device_name: Option<String>,
) -> Result<String, String> {
    let mut recorder = crate::lock_or_string_error(&state.recorder, "recorder")?;
    recorder.set_device(device_name.clone());
    crate::config::settings::set_selected_microphone(device_name)
        .map(|_| "マイクデバイスを設定しました".to_string())
        .map_err(|e| e.to_string())
}

// 現在選択中のマイクを取得コマンド
#[tauri::command]
pub(crate) fn get_selected_audio_device() -> Option<String> {
    crate::config::settings::get_selected_microphone()
}

// 言語設定コマンド
#[tauri::command]
pub(crate) fn get_language() -> String {
    crate::config::settings::get_language()
}

#[tauri::command]
pub(crate) fn set_language(lang: String) -> Result<String, String> {
    crate::config::settings::set_language(&lang)
        .map(|_| format!("言語を {} に設定しました", lang))
        .map_err(|e| e.to_string())
}

// 最短入力時間（ms）取得コマンド
#[tauri::command]
pub(crate) fn get_min_input_duration_ms() -> u64 {
    crate::config::settings::get_min_input_duration_ms()
}

// 最短入力時間（ms）設定コマンド
#[tauri::command]
pub(crate) fn set_min_input_duration_ms(duration_ms: u64) -> Result<(), String> {
    crate::config::settings::set_min_input_duration_ms(duration_ms).map_err(|e| e.to_string())
}

// Hybrid判定閾値（ms）取得コマンド
#[tauri::command]
pub(crate) fn get_hybrid_threshold_ms() -> u64 {
    crate::config::settings::get_hybrid_threshold_ms()
}

// Hybrid判定閾値（ms）設定コマンド
#[tauri::command]
pub(crate) fn set_hybrid_threshold_ms(duration_ms: u64) -> Result<(), String> {
    crate::config::settings::set_hybrid_threshold_ms(duration_ms).map_err(|e| e.to_string())
}

// ショートカット設定取得
#[tauri::command]
pub(crate) fn get_shortcut_settings() -> ShortcutSettings {
    crate::config::settings::get_shortcut_settings()
}

// ショートカット設定更新
#[tauri::command]
pub(crate) fn set_shortcut_settings(
    app: tauri::AppHandle,
    shortcuts: ShortcutSettings,
) -> Result<(), String> {
    let normalized = crate::config::settings::normalize_shortcuts(shortcuts);
    crate::config::settings::set_shortcut_settings(normalized.clone())
        .map_err(|e| e.to_string())?;

    crate::shortcut::register_global_shortcuts(&app, normalized)?;

    app.emit("shortcuts-updated", ())
        .map_err(|e| e.to_string())?;

    Ok(())
}

#[tauri::command]
pub(crate) fn pause_shortcuts(app: tauri::AppHandle) -> Result<(), String> {
    app.global_shortcut()
        .unregister_all()
        .map_err(|e| format!("ショートカットの一時停止に失敗しました: {}", e))?;
    crate::shortcut::reset_input_shortcut_state(&app);
    Ok(())
}

#[tauri::command]
pub(crate) fn resume_shortcuts(app: tauri::AppHandle) -> Result<(), String> {
    let shortcuts = crate::config::settings::get_shortcut_settings();
    crate::shortcut::register_global_shortcuts(&app, shortcuts)
}

// 履歴取得コマンド
#[tauri::command]
pub(crate) fn get_history() -> Vec<crate::history::HistoryItem> {
    crate::history::HistoryManager::load_history()
}

// 履歴クリアコマンド
#[tauri::command]
pub(crate) fn clear_history() -> Result<String, String> {
    crate::history::HistoryManager::clear_history().map(|_| "履歴をクリアしました".to_string())
}

// 累積スタッツ取得コマンド
#[tauri::command]
pub(crate) fn get_stats_snapshot() -> crate::history::StatsSnapshot {
    crate::history::HistoryManager::get_stats_snapshot()
}

// 日次使用量取得コマンド
#[tauri::command]
pub(crate) fn get_daily_usage(days: u16) -> Vec<crate::history::DailyUsagePoint> {
    crate::history::HistoryManager::get_daily_usage(days)
}

// 特定の履歴削除コマンド
#[tauri::command]
pub(crate) fn delete_history_item(id: u64) -> Result<String, String> {
    crate::history::HistoryManager::remove_entry(id).map(|_| "削除しました".to_string())
}

// ===== ユーザー辞書コマンド =====

#[tauri::command]
pub(crate) fn get_user_dictionary() -> Vec<crate::config::settings::DictionaryEntry> {
    crate::config::settings::get_user_dictionary()
}

#[tauri::command]
pub(crate) fn add_dictionary_entry(
    word: String,
) -> Result<crate::config::settings::DictionaryEntry, String> {
    crate::config::settings::add_dictionary_entry(&word).map_err(|e| e.to_string())
}

#[tauri::command]
pub(crate) fn remove_dictionary_entry(id: u64) -> Result<(), String> {
    crate::config::settings::remove_dictionary_entry(id).map_err(|e| e.to_string())
}

#[tauri::command]
pub(crate) fn clear_dictionary() -> Result<(), String> {
    crate::config::settings::clear_dictionary().map_err(|e| e.to_string())
}

// ===== エラーログコマンド =====

#[tauri::command]
pub(crate) fn get_error_log() -> Vec<crate::error_log::ErrorLogEntry> {
    crate::error_log::ErrorLogManager::load_error_log()
}

#[tauri::command]
pub(crate) fn clear_error_log() -> Result<(), String> {
    crate::error_log::ErrorLogManager::clear_error_log()
}
