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

// UI で提示するマイク感度のプリセット。保存値はこのいずれかに正規化される。
const MIC_SENSITIVITY_PRESETS: [f64; 3] = [0.5, 1.0, 3.0];

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
    // alias は旧設定ファイル（whisper-turbo 時代）との後方互換のため残す
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

impl SttProvider {
    /// 履歴などへ保存する識別子。serde の rename 値と一致させること。
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Gemini => "gemini",
            Self::Hybrid => "hybrid",
            Self::Collaborate => "collaborate",
            Self::Whisper => "whisper",
        }
    }

    /// as_str の逆変換。未知の値は None（旧設定の "whisper-turbo" は serde 側の alias で吸収する）。
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "gemini" => Some(Self::Gemini),
            "hybrid" => Some(Self::Hybrid),
            "collaborate" => Some(Self::Collaborate),
            "whisper" => Some(Self::Whisper),
            _ => None,
        }
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

/// プラットフォーム別設定（mac/win）を優先し、なければグローバル設定へフォールバックして読む。
fn read_setting<T>(
    pick_platform: impl FnOnce(&PlatformSettings) -> Option<T>,
    pick_global: impl FnOnce(AppSettings) -> Option<T>,
) -> Option<T> {
    let settings = load_settings();
    if let Some(value) = platform_settings(&settings).and_then(pick_platform) {
        return Some(value);
    }
    pick_global(settings)
}

/// プラットフォーム別設定（mac/win）があればそこへ、なければグローバル設定へ書いて保存する。
fn write_setting<T>(
    value: T,
    set_platform: impl FnOnce(&mut PlatformSettings, T),
    set_global: impl FnOnce(&mut AppSettings, T),
) -> Result<(), ConfigError> {
    let mut settings = load_settings();
    if let Some(platform) = platform_settings_mut(&mut settings) {
        set_platform(platform, value);
    } else {
        set_global(&mut settings, value);
    }
    save_settings(&settings)
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
    write_setting(
        provider,
        |p, v| p.stt_provider = Some(v),
        |s, v| s.stt_provider = Some(v),
    )
}

/// 使用する文字起こしプロバイダを取得
pub fn get_stt_provider() -> SttProvider {
    read_setting(|p| p.stt_provider, |s| s.stt_provider).unwrap_or(DEFAULT_STT_PROVIDER)
}

/// 使用するモデルを保存
pub fn set_model(model: &str) -> Result<(), ConfigError> {
    write_setting(
        model.to_string(),
        |p, v| p.model = Some(v),
        |s, v| s.model = Some(v),
    )
}

/// AI処理用モデルを保存
pub fn set_ai_model(model: &str) -> Result<(), ConfigError> {
    write_setting(
        model.to_string(),
        |p, v| p.ai_model = Some(v),
        |s, v| s.ai_model = Some(v),
    )
}

/// Whisperモデルパスを設定
pub fn set_whisper_model_path(model_path: Option<String>) -> Result<(), ConfigError> {
    write_setting(
        non_empty_trimmed(model_path),
        |p, v| p.whisper_model_path = v,
        |s, v| s.whisper_model_path = v,
    )
}

/// 使用するモデルを取得
pub fn get_model() -> String {
    read_setting(|p| p.model.clone(), |s| s.model)
        .unwrap_or_else(|| DEFAULT_MODEL.to_string())
}

/// AI処理用モデルを取得
pub fn get_ai_model() -> String {
    read_setting(|p| p.ai_model.clone(), |s| s.ai_model)
        .unwrap_or_else(|| DEFAULT_AI_MODEL.to_string())
}

/// 通知モードを取得
/// 通知機能は現在無効化しているため setter は提供しない。
/// get はトレイ表示・設定画面の状態表示用に残している。
pub fn get_notification_mode() -> FeedbackMode {
    read_setting(
        |p| p.notification_mode.clone(),
        |s| Some(s.notification_mode),
    )
    .unwrap_or_default()
}

/// 音声モードを設定
pub fn set_sound_mode(mode: FeedbackMode) -> Result<(), ConfigError> {
    write_setting(
        mode,
        |p, v| p.sound_mode = Some(v),
        |s, v| s.sound_mode = v,
    )
}

/// 音声モードを取得
pub fn get_sound_mode() -> FeedbackMode {
    read_setting(|p| p.sound_mode.clone(), |s| Some(s.sound_mode)).unwrap_or_default()
}

pub fn normalize_mic_sensitivity(value: f64) -> f64 {
    let mut best = MIC_SENSITIVITY_PRESETS[0];
    let mut best_distance = (value - best).abs();
    for preset in MIC_SENSITIVITY_PRESETS.iter().copied().skip(1) {
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
    normalize_mic_sensitivity(
        read_setting(|p| p.mic_sensitivity, |s| s.mic_sensitivity)
            .unwrap_or(DEFAULT_MIC_SENSITIVITY),
    )
}

pub fn set_mic_sensitivity(value: f64) -> Result<(), ConfigError> {
    write_setting(
        normalize_mic_sensitivity(value),
        |p, v| p.mic_sensitivity = Some(v),
        |s, v| s.mic_sensitivity = Some(v),
    )
}

pub fn get_wave_motion_scale() -> f64 {
    normalize_wave_motion_scale(
        read_setting(|p| p.wave_motion_scale, |s| s.wave_motion_scale)
            .unwrap_or(DEFAULT_WAVE_MOTION_SCALE),
    )
}

pub fn set_wave_motion_scale(value: f64) -> Result<(), ConfigError> {
    write_setting(
        normalize_wave_motion_scale(value),
        |p, v| p.wave_motion_scale = Some(v),
        |s, v| s.wave_motion_scale = Some(v),
    )
}

pub fn get_input_delivery_mode() -> InputDeliveryMode {
    read_setting(
        |p| p.input_delivery_mode.clone(),
        |s| Some(s.input_delivery_mode),
    )
    .unwrap_or_default()
}

pub fn set_input_delivery_mode(mode: InputDeliveryMode) -> Result<(), ConfigError> {
    write_setting(
        mode,
        |p, v| p.input_delivery_mode = Some(v),
        |s, v| s.input_delivery_mode = v,
    )
}

/// 選択中のマイクデバイスを設定
pub fn set_selected_microphone(device_name: Option<String>) -> Result<(), ConfigError> {
    write_setting(
        device_name,
        // TOML は null を表現できないため Some(None) を保存しない。
        // 未選択（None）は項目自体を未設定に戻す。
        |p, v| p.selected_microphone = v.map(Some),
        |s, v| s.selected_microphone = v,
    )
}

/// 選択中のマイクデバイスを取得
pub fn get_selected_microphone() -> Option<String> {
    read_setting(
        |p| p.selected_microphone.clone(),
        |s| Some(s.selected_microphone),
    )
    .flatten()
}

/// 言語を設定
pub fn set_language(language: &str) -> Result<(), ConfigError> {
    write_setting(
        language.to_string(),
        |p, v| p.language = Some(v),
        |s, v| s.language = Some(v),
    )
}

/// 言語を取得
pub fn get_language() -> String {
    read_setting(|p| p.language.clone(), |s| s.language).unwrap_or_else(|| "ja".to_string())
}

/// 最短入力時間（ms）を設定
pub fn set_min_input_duration_ms(duration_ms: u64) -> Result<(), ConfigError> {
    write_setting(
        duration_ms,
        |p, v| p.min_input_duration_ms = Some(v),
        |s, v| s.min_input_duration_ms = Some(v),
    )
}

/// 最短入力時間（ms）を取得
pub fn get_min_input_duration_ms() -> u64 {
    read_setting(|p| p.min_input_duration_ms, |s| s.min_input_duration_ms)
        .unwrap_or(DEFAULT_MIN_INPUT_DURATION_MS)
}

/// HybridモードのWhisper判定閾値（ms）を設定
pub fn set_hybrid_threshold_ms(duration_ms: u64) -> Result<(), ConfigError> {
    write_setting(
        duration_ms,
        |p, v| p.hybrid_threshold_ms = Some(v),
        |s, v| s.hybrid_threshold_ms = Some(v),
    )
}

/// HybridモードのWhisper判定閾値（ms）を取得
pub fn get_hybrid_threshold_ms() -> u64 {
    read_setting(|p| p.hybrid_threshold_ms, |s| s.hybrid_threshold_ms)
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
    let shortcuts =
        read_setting(|p| p.shortcuts.clone(), |s| Some(s.shortcuts)).unwrap_or_default();
    normalize_shortcuts_internal(shortcuts)
}

/// ショートカット設定を保存
pub fn set_shortcut_settings(shortcuts: ShortcutSettings) -> Result<(), ConfigError> {
    let validated = ShortcutSettings {
        input_shortcut: validate_shortcut(&shortcuts.input_shortcut)?,
        os_paste_shortcut: validate_shortcut(&shortcuts.os_paste_shortcut)?,
    };
    write_setting(
        validated,
        |p, v| p.shortcuts = Some(v),
        |s, v| s.shortcuts = v,
    )
}

/// プラットフォームに合わせてショートカットを正規化
pub fn normalize_shortcuts(shortcuts: ShortcutSettings) -> ShortcutSettings {
    normalize_shortcuts_internal(shortcuts)
}

pub fn get_whisper_cli_path() -> Option<String> {
    non_empty_trimmed(read_setting(
        |p| p.whisper_cli_path.clone(),
        |s| s.whisper_cli_path,
    ))
}

pub fn get_whisper_model_path() -> Option<String> {
    non_empty_trimmed(read_setting(
        |p| p.whisper_model_path.clone(),
        |s| s.whisper_model_path,
    ))
}

pub fn get_whisper_model_paths() -> Vec<String> {
    let values = read_setting(
        |p| p.whisper_model_paths.clone(),
        |s| s.whisper_model_paths,
    )
    .unwrap_or_default();
    normalize_path_list(values)
}

pub fn set_whisper_model_paths(paths: Vec<String>) -> Result<(), ConfigError> {
    write_setting(
        normalize_path_list(paths),
        |p, v| p.whisper_model_paths = Some(v),
        |s, v| s.whisper_model_paths = Some(v),
    )
}

pub fn get_whisper_output_dir() -> Option<String> {
    non_empty_trimmed(read_setting(
        |p| p.whisper_output_dir.clone(),
        |s| s.whisper_output_dir,
    ))
}

pub fn get_whisper_log_dir() -> Option<String> {
    non_empty_trimmed(read_setting(
        |p| p.whisper_log_dir.clone(),
        |s| s.whisper_log_dir,
    ))
}

pub fn get_whisper_language() -> Option<String> {
    let value = read_setting(
        |p| p.whisper_language.clone(),
        |s| s.whisper_language,
    );
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
    non_empty_trimmed(read_setting(
        |p| p.whisper_transcripts_path.clone(),
        |s| s.whisper_transcripts_path,
    ))
}

// onboarding 状態は OS ごとに独立して管理する（グローバル設定へフォールバックしない）。
// 同じ config.toml を mac/win で共有しても、初回セットアップは各 OS で個別に完了させたいため。

pub fn get_onboarding_ack_unnotarized() -> bool {
    read_setting(|p| p.onboarding_ack_unnotarized, |_| None).unwrap_or(false)
}

pub fn set_onboarding_ack_unnotarized(ack: bool) -> Result<(), ConfigError> {
    write_setting(ack, |p, v| p.onboarding_ack_unnotarized = Some(v), |_, _| {})
}

pub fn get_onboarding_completed_at() -> Option<i64> {
    read_setting(|p| p.onboarding_completed_at, |_| None)
}

pub fn set_onboarding_completed_now() -> Result<(), ConfigError> {
    let now = crate::util::current_unix_timestamp_secs() as i64;
    write_setting(now, |p, v| p.onboarding_completed_at = Some(v), |_, _| {})
}

pub fn get_onboarding_permission_probe_ok() -> Option<bool> {
    read_setting(|p| p.onboarding_permission_probe_ok, |_| None)
}

#[cfg(target_os = "macos")]
pub fn set_onboarding_permission_probe_ok(ok: bool) -> Result<(), ConfigError> {
    write_setting(
        ok,
        |p, v| p.onboarding_permission_probe_ok = Some(v),
        |_, _| {},
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- validate_shortcut: ショートカット文字列の正規化仕様 ---

    #[test]
    fn validate_shortcut_normalizes_alias_names() {
        assert_eq!(validate_shortcut("ctrl+shift+x").unwrap(), "Control+Shift+X");
        assert_eq!(validate_shortcut("cmd+v").unwrap(), "Super+V");
        assert_eq!(validate_shortcut("alt+space").unwrap(), "Option+space");
    }

    #[test]
    fn validate_shortcut_keeps_modifier_order_and_dedupes() {
        assert_eq!(
            validate_shortcut("shift+ctrl+shift+a").unwrap(),
            "Shift+Control+A"
        );
    }

    #[test]
    fn validate_shortcut_trims_whitespace_and_nbsp() {
        assert_eq!(
            validate_shortcut(" Option +\u{00a0}Space ").unwrap(),
            "Option+Space"
        );
    }

    #[test]
    fn validate_shortcut_rejects_modifier_only() {
        assert!(validate_shortcut("Control+Shift").is_err());
    }

    #[test]
    fn validate_shortcut_rejects_empty() {
        assert!(validate_shortcut(" + ").is_err());
        assert!(validate_shortcut("").is_err());
    }

    // --- 不正なショートカット設定はプラットフォーム既定値へフォールバックする ---

    #[test]
    fn invalid_saved_shortcut_falls_back_to_platform_default() {
        let normalized = normalize_shortcuts_internal(ShortcutSettings {
            input_shortcut: "Shift".to_string(),
            os_paste_shortcut: "Control+Shift+V".to_string(),
        });
        assert_eq!(normalized.input_shortcut, default_input_shortcut());
        assert_eq!(normalized.os_paste_shortcut, "Control+Shift+V");
    }

    // --- マイク感度はプリセット（0.5 / 1.0 / 3.0）へスナップする ---

    #[test]
    fn mic_sensitivity_snaps_to_nearest_preset() {
        assert_eq!(normalize_mic_sensitivity(0.4), 0.5);
        assert_eq!(normalize_mic_sensitivity(0.8), 1.0);
        assert_eq!(normalize_mic_sensitivity(2.4), 3.0);
        assert_eq!(normalize_mic_sensitivity(100.0), 3.0);
        assert_eq!(normalize_mic_sensitivity(-5.0), 0.5);
    }

    // --- 波形モーションスケールは四捨五入後 1〜10 にクランプする ---

    #[test]
    fn wave_motion_scale_rounds_and_clamps() {
        assert_eq!(normalize_wave_motion_scale(0.2), 1.0);
        assert_eq!(normalize_wave_motion_scale(5.4), 5.0);
        assert_eq!(normalize_wave_motion_scale(5.6), 6.0);
        assert_eq!(normalize_wave_motion_scale(99.0), 10.0);
    }

    // --- パスリストは空白除去・空要素除外・重複排除して順序を保持する ---

    #[test]
    fn normalize_path_list_dedupes_and_drops_blank() {
        let result = normalize_path_list(vec![
            " /a/model.bin ".to_string(),
            "".to_string(),
            "/b/model.bin".to_string(),
            "/a/model.bin".to_string(),
        ]);
        assert_eq!(result, vec!["/a/model.bin", "/b/model.bin"]);
    }

    // --- SttProvider::as_str は serde の rename 値（設定ファイル上の表記）と一致する ---

    #[test]
    fn stt_provider_as_str_matches_serde_rename() {
        for provider in [
            SttProvider::Gemini,
            SttProvider::Hybrid,
            SttProvider::Collaborate,
            SttProvider::Whisper,
        ] {
            let serialized = serde_json::to_string(&provider).unwrap();
            assert_eq!(serialized, format!("\"{}\"", provider.as_str()));
            assert_eq!(SttProvider::parse(provider.as_str()), Some(provider));
        }
        assert_eq!(SttProvider::parse("unknown"), None);
    }

    #[test]
    fn non_empty_trimmed_filters_blank_values() {
        assert_eq!(non_empty_trimmed(Some("  ".to_string())), None);
        assert_eq!(non_empty_trimmed(None), None);
        assert_eq!(
            non_empty_trimmed(Some(" /path ".to_string())),
            Some("/path".to_string())
        );
    }
}
