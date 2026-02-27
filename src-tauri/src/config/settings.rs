use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};
use thiserror::Error;
use tracing::error;

const DEFAULT_MODEL: &str = "gemini-2.5-flash-lite";
const DEFAULT_AI_MODEL: &str = "gemini-2.5-flash-lite";
const DEFAULT_STT_PROVIDER: SttProvider = SttProvider::Gemini;
const DEFAULT_MIN_INPUT_DURATION_MS: u64 = 2000;
const DEFAULT_HYBRID_THRESHOLD_MS: u64 = 10_000;
const MAX_DICTIONARY_ENTRIES: usize = 800;
const DEFAULT_MIC_SENSITIVITY: f64 = 1.0;
const DEFAULT_WAVE_MOTION_SCALE: f64 = 1.0;

fn sanitize_token(token: &str) -> String {
    token
        .trim_matches(|c: char| c.is_whitespace() || c == '\u{00a0}')
        .to_string()
}

fn default_input_shortcut() -> String {
    if cfg!(target_os = "macos") {
        "Option+Space".to_string()
    } else {
        "Control+Shift+X".to_string()
    }
}

fn default_os_paste_shortcut() -> String {
    if cfg!(target_os = "macos") {
        "Option+Control+V".to_string()
    } else {
        "Control+Shift+V".to_string()
    }
}

#[derive(Error, Debug)]
pub enum ConfigError {
    #[error("設定の保存に失敗しました: {0}")]
    SaveError(String),

    #[error("APIキーが設定されていません")]
    NotFound,

    #[error("辞書が満杯です（最大{0}件）")]
    DictionaryFull(usize),

    #[error("ショートカットが不正です: {0}")]
    InvalidShortcut(String),
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct DictionaryEntry {
    pub id: u64,
    pub word: String,
    pub created_at: i64,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub enum FeedbackMode {
    AlwaysOff,
    AlwaysOn,
    FullscreenOnly,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub enum InputDeliveryMode {
    #[serde(rename = "clipboard")]
    Clipboard,
    #[serde(rename = "type")]
    Type,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
pub enum SttProvider {
    #[serde(rename = "gemini")]
    Gemini,
    #[serde(rename = "hybrid")]
    Hybrid,
    #[serde(rename = "collaborate")]
    Collaborate,
    #[serde(rename = "whisper", alias = "whisper-turbo")]
    Whisper,
}

impl Default for FeedbackMode {
    fn default() -> Self {
        Self::FullscreenOnly
    }
}

impl Default for InputDeliveryMode {
    fn default() -> Self {
        Self::Type
    }
}

impl Default for SttProvider {
    fn default() -> Self {
        DEFAULT_STT_PROVIDER
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ShortcutSettings {
    pub input_shortcut: String,
    pub os_paste_shortcut: String,
}

impl Default for ShortcutSettings {
    fn default() -> Self {
        Self {
            input_shortcut: default_input_shortcut(),
            os_paste_shortcut: default_os_paste_shortcut(),
        }
    }
}

#[derive(Serialize, Deserialize, Default)]
struct AppSettings {
    #[serde(default)]
    stt_provider: Option<SttProvider>,
    model: Option<String>,
    ai_model: Option<String>,
    #[serde(default)]
    notification_mode: FeedbackMode,
    #[serde(default)]
    sound_mode: FeedbackMode,
    selected_microphone: Option<String>,
    language: Option<String>,
    #[serde(default)]
    min_input_duration_ms: Option<u64>,
    #[serde(default)]
    hybrid_threshold_ms: Option<u64>,
    #[serde(default)]
    shortcuts: ShortcutSettings,
    #[serde(default)]
    mic_sensitivity: Option<f64>,
    #[serde(default)]
    wave_motion_scale: Option<f64>,
    #[serde(default)]
    input_delivery_mode: InputDeliveryMode,
    #[serde(default)]
    whisper_cli_path: Option<String>,
    #[serde(default)]
    whisper_model_path: Option<String>,
    #[serde(default)]
    whisper_model_paths: Option<Vec<String>>,
    #[serde(default)]
    whisper_output_dir: Option<String>,
    #[serde(default)]
    whisper_log_dir: Option<String>,
    #[serde(default)]
    whisper_language: Option<String>,
    #[serde(default)]
    whisper_transcripts_path: Option<String>,
    #[serde(default)]
    mac: PlatformSettings,
    #[serde(default)]
    win: PlatformSettings,
}

#[derive(Serialize, Deserialize, Default, Clone)]
struct PlatformSettings {
    #[serde(default)]
    stt_provider: Option<SttProvider>,
    #[serde(default)]
    model: Option<String>,
    #[serde(default)]
    ai_model: Option<String>,
    #[serde(default)]
    notification_mode: Option<FeedbackMode>,
    #[serde(default)]
    sound_mode: Option<FeedbackMode>,
    #[serde(default)]
    selected_microphone: Option<Option<String>>,
    #[serde(default)]
    language: Option<String>,
    #[serde(default)]
    min_input_duration_ms: Option<u64>,
    #[serde(default)]
    hybrid_threshold_ms: Option<u64>,
    #[serde(default)]
    shortcuts: Option<ShortcutSettings>,
    #[serde(default)]
    mic_sensitivity: Option<f64>,
    #[serde(default)]
    wave_motion_scale: Option<f64>,
    #[serde(default)]
    input_delivery_mode: Option<InputDeliveryMode>,
    #[serde(default)]
    whisper_cli_path: Option<String>,
    #[serde(default)]
    whisper_model_path: Option<String>,
    #[serde(default)]
    whisper_model_paths: Option<Vec<String>>,
    #[serde(default)]
    whisper_output_dir: Option<String>,
    #[serde(default)]
    whisper_log_dir: Option<String>,
    #[serde(default)]
    whisper_language: Option<String>,
    #[serde(default)]
    whisper_transcripts_path: Option<String>,
    #[serde(default)]
    onboarding_ack_unnotarized: Option<bool>,
    #[serde(default)]
    onboarding_completed_at: Option<i64>,
    #[serde(default)]
    onboarding_permission_probe_ok: Option<bool>,
}

fn platform_settings(settings: &AppSettings) -> Option<&PlatformSettings> {
    #[cfg(target_os = "macos")]
    {
        return Some(&settings.mac);
    }

    #[cfg(target_os = "windows")]
    {
        return Some(&settings.win);
    }

    #[allow(unreachable_code)]
    None
}

fn platform_settings_mut(settings: &mut AppSettings) -> Option<&mut PlatformSettings> {
    #[cfg(target_os = "macos")]
    {
        return Some(&mut settings.mac);
    }

    #[cfg(target_os = "windows")]
    {
        return Some(&mut settings.win);
    }

    #[allow(unreachable_code)]
    None
}

pub fn get_app_home_dir() -> PathBuf {
    if let Ok(path) = std::env::var("KOETYPE_HOME") {
        let p = PathBuf::from(path);
        let _ = fs::create_dir_all(&p);
        return p;
    }

    let mut path = dirs::home_dir().unwrap_or_else(|| {
        error!("ホームディレクトリが取得できません。カレントディレクトリを代替として使用します。");
        PathBuf::from(".")
    });
    path.push(".config");
    path.push("koetype");
    let _ = fs::create_dir_all(&path);
    path
}

fn get_config_path() -> PathBuf {
    get_app_home_dir().join("config.toml")
}

fn load_settings() -> AppSettings {
    let path = get_config_path();
    if path.exists() {
        match fs::read_to_string(&path) {
            Err(e) => {
                error!(
                    "config.toml 読み込み失敗 ({}): {} — デフォルト設定で起動します",
                    path.display(),
                    e
                );
            }
            Ok(content) => match toml::from_str(&content) {
                Ok(settings) => return settings,
                Err(e) => {
                    error!(
                        "config.toml パース失敗 ({}): {} — デフォルト設定で起動します",
                        path.display(),
                        e
                    );
                }
            },
        }
    }

    AppSettings::default()
}

fn save_settings(settings: &AppSettings) -> Result<(), ConfigError> {
    let path = get_config_path();
    let content =
        toml::to_string_pretty(settings).map_err(|e| ConfigError::SaveError(e.to_string()))?;
    fs::write(path, content).map_err(|e| ConfigError::SaveError(e.to_string()))?;
    Ok(())
}

fn in_memory_api_key_store() -> &'static Mutex<Option<String>> {
    static API_KEY: OnceLock<Mutex<Option<String>>> = OnceLock::new();
    API_KEY.get_or_init(|| {
        let loaded = load_api_key_internal();
        Mutex::new(loaded)
    })
}

fn get_api_key_path() -> PathBuf {
    get_app_home_dir().join("api_key.json")
}

fn load_api_key_internal() -> Option<String> {
    let path = get_api_key_path();
    if !path.exists() {
        return None;
    }
    match fs::read_to_string(&path) {
        Ok(content) => match serde_json::from_str::<serde_json::Value>(&content) {
            Ok(v) => v["api_key"].as_str().map(|s| s.to_string()),
            Err(_) => None,
        },
        Err(_) => None,
    }
}

fn save_api_key_internal(api_key: &str) -> Result<(), ConfigError> {
    let path = get_api_key_path();
    let data = serde_json::json!({ "api_key": api_key });
    let content = serde_json::to_string_pretty(&data).map_err(|e| {
        ConfigError::SaveError(format!("APIキーのシリアライズに失敗しました: {}", e))
    })?;
    fs::write(path, content)
        .map_err(|e| ConfigError::SaveError(format!("APIキーの保存に失敗しました: {}", e)))?;
    Ok(())
}

fn get_user_dictionary_path() -> PathBuf {
    get_app_home_dir().join("user_dictionary.json")
}

fn load_user_dictionary_internal() -> Vec<DictionaryEntry> {
    let path = get_user_dictionary_path();
    if !path.exists() {
        return Vec::new();
    }
    match fs::read_to_string(&path) {
        Err(e) => {
            error!(
                "ユーザー辞書読み込み失敗 ({}): {} — 辞書を空として扱います",
                path.display(),
                e
            );
            Vec::new()
        }
        Ok(content) => match serde_json::from_str(&content) {
            Ok(entries) => entries,
            Err(e) => {
                error!(
                    "ユーザー辞書パース失敗 ({}): {} — 辞書を空として扱います",
                    path.display(),
                    e
                );
                Vec::new()
            }
        },
    }
}

fn save_user_dictionary_internal(entries: &[DictionaryEntry]) -> Result<(), ConfigError> {
    let path = get_user_dictionary_path();
    let content =
        serde_json::to_string_pretty(entries).map_err(|e| ConfigError::SaveError(e.to_string()))?;
    fs::write(path, content).map_err(|e| ConfigError::SaveError(e.to_string()))?;
    Ok(())
}

/// APIキーを保存
pub fn set_api_key(api_key: &str) -> Result<(), ConfigError> {
    let trimmed = api_key.trim();
    if trimmed.is_empty() {
        let path = get_api_key_path();
        if path.exists() {
            let _ = fs::remove_file(path);
        }
    } else {
        save_api_key_internal(trimmed)?;
    }

    let mut guard = in_memory_api_key_store()
        .lock()
        .map_err(|_| ConfigError::SaveError("APIキーのメモリ保存に失敗しました".to_string()))?;
    *guard = if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    };
    Ok(())
}

/// APIキーを取得
pub fn get_api_key() -> Result<String, ConfigError> {
    if let Ok(v) = std::env::var("KOETYPE_GEMINI_API_KEY") {
        let trimmed = v.trim().to_string();
        if !trimmed.is_empty() {
            return Ok(trimmed);
        }
    }
    if let Ok(v) = std::env::var("GEMINI_API_KEY") {
        let trimmed = v.trim().to_string();
        if !trimmed.is_empty() {
            return Ok(trimmed);
        }
    }
    in_memory_api_key_store()
        .lock()
        .ok()
        .and_then(|guard| guard.clone())
        .filter(|v| !v.trim().is_empty())
        .ok_or(ConfigError::NotFound)
}

/// 使用する文字起こしプロバイダを保存
pub fn set_stt_provider(provider: SttProvider) -> Result<(), ConfigError> {
    let mut settings = load_settings();
    if let Some(platform) = platform_settings_mut(&mut settings) {
        platform.stt_provider = Some(provider);
    } else {
        settings.stt_provider = Some(provider);
    }
    save_settings(&settings)
}

/// 使用する文字起こしプロバイダを取得
pub fn get_stt_provider() -> SttProvider {
    let settings = load_settings();
    platform_settings(&settings)
        .and_then(|p| p.stt_provider)
        .or(settings.stt_provider)
        .unwrap_or(DEFAULT_STT_PROVIDER)
}

/// 使用するモデルを保存
pub fn set_model(model: &str) -> Result<(), ConfigError> {
    let mut settings = load_settings();
    if let Some(platform) = platform_settings_mut(&mut settings) {
        platform.model = Some(model.to_string());
    } else {
        settings.model = Some(model.to_string());
    }
    save_settings(&settings)
}

/// AI処理用モデルを保存
pub fn set_ai_model(model: &str) -> Result<(), ConfigError> {
    let mut settings = load_settings();
    if let Some(platform) = platform_settings_mut(&mut settings) {
        platform.ai_model = Some(model.to_string());
    } else {
        settings.ai_model = Some(model.to_string());
    }
    save_settings(&settings)
}

/// Whisperモデルパスを設定
pub fn set_whisper_model_path(model_path: Option<String>) -> Result<(), ConfigError> {
    let normalized = non_empty_trimmed(model_path);
    let mut settings = load_settings();
    if let Some(platform) = platform_settings_mut(&mut settings) {
        platform.whisper_model_path = normalized.clone();
    } else {
        settings.whisper_model_path = normalized;
    }
    save_settings(&settings)
}

/// 使用するモデルを取得
pub fn get_model() -> String {
    let settings = load_settings();
    platform_settings(&settings)
        .and_then(|p| p.model.clone())
        .or(settings.model)
        .unwrap_or_else(|| DEFAULT_MODEL.to_string())
}

/// AI処理用モデルを取得
pub fn get_ai_model() -> String {
    let settings = load_settings();
    platform_settings(&settings)
        .and_then(|p| p.ai_model.clone())
        .or(settings.ai_model)
        .unwrap_or_else(|| DEFAULT_AI_MODEL.to_string())
}

/// 通知モードを設定 - 通知機能を無効化しているためコメントアウト
// pub fn set_notification_mode(mode: FeedbackMode) -> Result<(), ConfigError> {
//     let mut settings = load_settings();
//     if let Some(platform) = platform_settings_mut(&mut settings) {
//         platform.notification_mode = Some(mode.clone());
//     } else {
//         settings.notification_mode = mode;
//     }
//     save_settings(&settings)
// }

/// 通知モードを取得
pub fn get_notification_mode() -> FeedbackMode {
    let settings = load_settings();
    platform_settings(&settings)
        .and_then(|p| p.notification_mode.clone())
        .unwrap_or(settings.notification_mode)
}

/// 音声モードを設定
pub fn set_sound_mode(mode: FeedbackMode) -> Result<(), ConfigError> {
    let mut settings = load_settings();
    if let Some(platform) = platform_settings_mut(&mut settings) {
        platform.sound_mode = Some(mode.clone());
    } else {
        settings.sound_mode = mode;
    }
    save_settings(&settings)
}

/// 音声モードを取得
pub fn get_sound_mode() -> FeedbackMode {
    let settings = load_settings();
    platform_settings(&settings)
        .and_then(|p| p.sound_mode.clone())
        .unwrap_or(settings.sound_mode)
}

pub fn normalize_mic_sensitivity(value: f64) -> f64 {
    let presets = [0.5_f64, 1.0_f64, 3.0_f64];
    let mut best = presets[0];
    let mut best_distance = (value - best).abs();
    for preset in presets.iter().copied().skip(1) {
        let distance = (value - preset).abs();
        if distance < best_distance {
            best = preset;
            best_distance = distance;
        }
    }
    best
}

pub fn normalize_wave_motion_scale(value: f64) -> f64 {
    value.round().clamp(1.0, 10.0)
}

pub fn get_mic_sensitivity() -> f64 {
    let settings = load_settings();
    normalize_mic_sensitivity(
        platform_settings(&settings)
            .and_then(|p| p.mic_sensitivity)
            .or(settings.mic_sensitivity)
            .unwrap_or(DEFAULT_MIC_SENSITIVITY),
    )
}

pub fn set_mic_sensitivity(value: f64) -> Result<(), ConfigError> {
    let mut settings = load_settings();
    let normalized = normalize_mic_sensitivity(value);
    if let Some(platform) = platform_settings_mut(&mut settings) {
        platform.mic_sensitivity = Some(normalized);
    } else {
        settings.mic_sensitivity = Some(normalized);
    }
    save_settings(&settings)
}

pub fn get_wave_motion_scale() -> f64 {
    let settings = load_settings();
    normalize_wave_motion_scale(
        platform_settings(&settings)
            .and_then(|p| p.wave_motion_scale)
            .or(settings.wave_motion_scale)
            .unwrap_or(DEFAULT_WAVE_MOTION_SCALE),
    )
}

pub fn set_wave_motion_scale(value: f64) -> Result<(), ConfigError> {
    let mut settings = load_settings();
    let normalized = normalize_wave_motion_scale(value);
    if let Some(platform) = platform_settings_mut(&mut settings) {
        platform.wave_motion_scale = Some(normalized);
    } else {
        settings.wave_motion_scale = Some(normalized);
    }
    save_settings(&settings)
}

pub fn get_input_delivery_mode() -> InputDeliveryMode {
    let settings = load_settings();
    platform_settings(&settings)
        .and_then(|p| p.input_delivery_mode.clone())
        .unwrap_or(settings.input_delivery_mode)
}

pub fn set_input_delivery_mode(mode: InputDeliveryMode) -> Result<(), ConfigError> {
    let mut settings = load_settings();
    if let Some(platform) = platform_settings_mut(&mut settings) {
        platform.input_delivery_mode = Some(mode.clone());
    } else {
        settings.input_delivery_mode = mode;
    }
    save_settings(&settings)
}

/// 選択中のマイクデバイスを設定
pub fn set_selected_microphone(device_name: Option<String>) -> Result<(), ConfigError> {
    let mut settings = load_settings();
    if let Some(platform) = platform_settings_mut(&mut settings) {
        // TOML は null を表現できないため Some(None) を保存しない。
        // 未選択（None）は項目自体を未設定に戻す。
        platform.selected_microphone = device_name.map(Some);
    } else {
        settings.selected_microphone = device_name;
    }
    save_settings(&settings)
}

/// 選択中のマイクデバイスを取得
pub fn get_selected_microphone() -> Option<String> {
    let settings = load_settings();
    if let Some(value) = platform_settings(&settings).and_then(|p| p.selected_microphone.clone()) {
        return value;
    }
    settings.selected_microphone
}

/// 言語を設定
pub fn set_language(language: &str) -> Result<(), ConfigError> {
    let mut settings = load_settings();
    if let Some(platform) = platform_settings_mut(&mut settings) {
        platform.language = Some(language.to_string());
    } else {
        settings.language = Some(language.to_string());
    }
    save_settings(&settings)
}

/// 言語を取得
pub fn get_language() -> String {
    let settings = load_settings();
    platform_settings(&settings)
        .and_then(|p| p.language.clone())
        .or(settings.language)
        .unwrap_or_else(|| "ja".to_string())
}

/// 最短入力時間（ms）を設定
pub fn set_min_input_duration_ms(duration_ms: u64) -> Result<(), ConfigError> {
    let mut settings = load_settings();
    if let Some(platform) = platform_settings_mut(&mut settings) {
        platform.min_input_duration_ms = Some(duration_ms);
    } else {
        settings.min_input_duration_ms = Some(duration_ms);
    }
    save_settings(&settings)
}

/// 最短入力時間（ms）を取得
pub fn get_min_input_duration_ms() -> u64 {
    let settings = load_settings();
    platform_settings(&settings)
        .and_then(|p| p.min_input_duration_ms)
        .or(settings.min_input_duration_ms)
        .unwrap_or(DEFAULT_MIN_INPUT_DURATION_MS)
}

/// HybridモードのWhisper判定閾値（ms）を設定
pub fn set_hybrid_threshold_ms(duration_ms: u64) -> Result<(), ConfigError> {
    let mut settings = load_settings();
    if let Some(platform) = platform_settings_mut(&mut settings) {
        platform.hybrid_threshold_ms = Some(duration_ms);
    } else {
        settings.hybrid_threshold_ms = Some(duration_ms);
    }
    save_settings(&settings)
}

/// HybridモードのWhisper判定閾値（ms）を取得
pub fn get_hybrid_threshold_ms() -> u64 {
    let settings = load_settings();
    platform_settings(&settings)
        .and_then(|p| p.hybrid_threshold_ms)
        .or(settings.hybrid_threshold_ms)
        .unwrap_or(DEFAULT_HYBRID_THRESHOLD_MS)
}

/// ユーザー辞書を取得
pub fn get_user_dictionary() -> Vec<DictionaryEntry> {
    load_user_dictionary_internal()
}

/// ユーザー辞書に単語を追加
pub fn add_dictionary_entry(word: &str) -> Result<DictionaryEntry, ConfigError> {
    let mut entries = load_user_dictionary_internal();

    if entries.len() >= MAX_DICTIONARY_ENTRIES {
        return Err(ConfigError::DictionaryFull(MAX_DICTIONARY_ENTRIES));
    }

    let now = crate::util::current_unix_timestamp_secs();
    let next_id = entries
        .iter()
        .map(|entry| entry.id)
        .max()
        .unwrap_or(0)
        .saturating_add(1);

    let entry = DictionaryEntry {
        id: next_id,
        word: word.to_string(),
        created_at: now as i64,
    };

    entries.insert(0, entry.clone());
    save_user_dictionary_internal(&entries)?;

    Ok(entry)
}

/// ユーザー辞書から単語を削除
pub fn remove_dictionary_entry(id: u64) -> Result<(), ConfigError> {
    let mut entries = load_user_dictionary_internal();
    entries.retain(|entry| entry.id != id);
    save_user_dictionary_internal(&entries)
}

/// ユーザー辞書をクリア
pub fn clear_dictionary() -> Result<(), ConfigError> {
    save_user_dictionary_internal(&[])
}

fn normalize_token_name(token: &str) -> String {
    match token.to_lowercase().as_str() {
        "ctrl" | "control" => "Control".to_string(),
        "cmd" | "command" | "super" | "meta" => "Super".to_string(),
        "opt" | "option" | "alt" => "Option".to_string(),
        "shift" => "Shift".to_string(),
        other => {
            // Keep letter casing for readability (make single letters uppercase)
            if other.len() == 1 {
                other.to_uppercase()
            } else {
                token.to_string()
            }
        }
    }
}

pub fn validate_shortcut(shortcut: &str) -> Result<String, ConfigError> {
    let tokens: Vec<String> = shortcut
        .split('+')
        .map(|t| sanitize_token(t))
        .filter(|t| !t.is_empty())
        .map(|t| normalize_token_name(&t))
        .collect();

    if tokens.is_empty() {
        return Err(ConfigError::InvalidShortcut("キーが空です".to_string()));
    }

    let mut modifiers: Vec<String> = Vec::new();
    let mut mains: Vec<String> = Vec::new();

    for t in tokens {
        match t.as_str() {
            "Control" | "Super" | "Option" | "Shift" => {
                if !modifiers.contains(&t) {
                    modifiers.push(t);
                }
            }
            _ => mains.push(t),
        }
    }

    if mains.is_empty() {
        return Err(ConfigError::InvalidShortcut(
            "修飾キーのみのショートカットは設定できません".to_string(),
        ));
    }

    // Preserve user order: modifiers first in the order they were provided, then main keys.
    let mut result = modifiers;
    result.extend(mains);

    Ok(result.join("+"))
}

fn normalize_shortcuts_internal(settings: ShortcutSettings) -> ShortcutSettings {
    let input = match validate_shortcut(&settings.input_shortcut) {
        Ok(v) => v,
        Err(_) => default_input_shortcut(),
    };
    let os_paste = match validate_shortcut(&settings.os_paste_shortcut) {
        Ok(v) => v,
        Err(_) => default_os_paste_shortcut(),
    };
    ShortcutSettings {
        input_shortcut: input,
        os_paste_shortcut: os_paste,
    }
}

fn non_empty_trimmed(value: Option<String>) -> Option<String> {
    value
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

fn normalize_path_list(values: Vec<String>) -> Vec<String> {
    let mut normalized = Vec::new();
    for value in values {
        let trimmed = value.trim();
        if trimmed.is_empty() {
            continue;
        }
        let owned = trimmed.to_string();
        if !normalized.contains(&owned) {
            normalized.push(owned);
        }
    }
    normalized
}

/// ショートカット設定を取得
pub fn get_shortcut_settings() -> ShortcutSettings {
    let settings = load_settings();
    let shortcuts = platform_settings(&settings)
        .and_then(|p| p.shortcuts.clone())
        .unwrap_or(settings.shortcuts);
    normalize_shortcuts_internal(shortcuts)
}

/// ショートカット設定を保存
pub fn set_shortcut_settings(shortcuts: ShortcutSettings) -> Result<(), ConfigError> {
    let validated = ShortcutSettings {
        input_shortcut: validate_shortcut(&shortcuts.input_shortcut)?,
        os_paste_shortcut: validate_shortcut(&shortcuts.os_paste_shortcut)?,
    };
    let mut settings = load_settings();
    if let Some(platform) = platform_settings_mut(&mut settings) {
        platform.shortcuts = Some(validated);
    } else {
        settings.shortcuts = validated;
    }
    save_settings(&settings)
}

/// プラットフォームに合わせてショートカットを正規化
pub fn normalize_shortcuts(shortcuts: ShortcutSettings) -> ShortcutSettings {
    normalize_shortcuts_internal(shortcuts)
}

pub fn get_whisper_cli_path() -> Option<String> {
    let settings = load_settings();
    let value = platform_settings(&settings)
        .and_then(|p| p.whisper_cli_path.clone())
        .or(settings.whisper_cli_path);
    non_empty_trimmed(value)
}

pub fn get_whisper_model_path() -> Option<String> {
    let settings = load_settings();
    let value = platform_settings(&settings)
        .and_then(|p| p.whisper_model_path.clone())
        .or(settings.whisper_model_path);
    non_empty_trimmed(value)
}

pub fn get_whisper_model_paths() -> Vec<String> {
    let settings = load_settings();
    let values = platform_settings(&settings)
        .and_then(|p| p.whisper_model_paths.clone())
        .or(settings.whisper_model_paths)
        .unwrap_or_default();
    normalize_path_list(values)
}

pub fn set_whisper_model_paths(paths: Vec<String>) -> Result<(), ConfigError> {
    let normalized = normalize_path_list(paths);
    let mut settings = load_settings();
    if let Some(platform) = platform_settings_mut(&mut settings) {
        platform.whisper_model_paths = Some(normalized);
    } else {
        settings.whisper_model_paths = Some(normalized);
    }
    save_settings(&settings)
}

pub fn get_whisper_output_dir() -> Option<String> {
    let settings = load_settings();
    let value = platform_settings(&settings)
        .and_then(|p| p.whisper_output_dir.clone())
        .or(settings.whisper_output_dir);
    non_empty_trimmed(value)
}

pub fn get_whisper_log_dir() -> Option<String> {
    let settings = load_settings();
    let value = platform_settings(&settings)
        .and_then(|p| p.whisper_log_dir.clone())
        .or(settings.whisper_log_dir);
    non_empty_trimmed(value)
}

pub fn get_whisper_language() -> Option<String> {
    let settings = load_settings();
    let value = platform_settings(&settings)
        .and_then(|p| p.whisper_language.clone())
        .or(settings.whisper_language);
    let lang = non_empty_trimmed(value)?;
    match lang.as_str() {
        "auto" | "ja" | "en" => Some(lang),
        _ => None,
    }
}

/// Whisper用の言語コードを解決する。
/// whisper_language 設定がある場合はそれを使い、なければ get_language() から変換する。
pub fn resolve_whisper_language() -> String {
    if let Some(lang) = get_whisper_language() {
        return lang;
    }
    match get_language().as_str() {
        "ja" => "ja".to_string(),
        "en" => "en".to_string(),
        _ => "auto".to_string(),
    }
}

pub fn get_whisper_transcripts_path() -> Option<String> {
    let settings = load_settings();
    let value = platform_settings(&settings)
        .and_then(|p| p.whisper_transcripts_path.clone())
        .or(settings.whisper_transcripts_path);
    non_empty_trimmed(value)
}

pub fn get_onboarding_ack_unnotarized() -> bool {
    let settings = load_settings();
    platform_settings(&settings)
        .and_then(|p| p.onboarding_ack_unnotarized)
        .unwrap_or(false)
}

pub fn set_onboarding_ack_unnotarized(ack: bool) -> Result<(), ConfigError> {
    let mut settings = load_settings();
    if let Some(platform) = platform_settings_mut(&mut settings) {
        platform.onboarding_ack_unnotarized = Some(ack);
    }
    save_settings(&settings)
}

pub fn get_onboarding_completed_at() -> Option<i64> {
    let settings = load_settings();
    platform_settings(&settings).and_then(|p| p.onboarding_completed_at)
}

pub fn set_onboarding_completed_now() -> Result<(), ConfigError> {
    let mut settings = load_settings();
    let now = crate::util::current_unix_timestamp_secs() as i64;
    if let Some(platform) = platform_settings_mut(&mut settings) {
        platform.onboarding_completed_at = Some(now);
    }
    save_settings(&settings)
}

pub fn get_onboarding_permission_probe_ok() -> Option<bool> {
    let settings = load_settings();
    platform_settings(&settings).and_then(|p| p.onboarding_permission_probe_ok)
}

pub fn set_onboarding_permission_probe_ok(ok: bool) -> Result<(), ConfigError> {
    let mut settings = load_settings();
    if let Some(platform) = platform_settings_mut(&mut settings) {
        platform.onboarding_permission_probe_ok = Some(ok);
    }
    save_settings(&settings)
}
