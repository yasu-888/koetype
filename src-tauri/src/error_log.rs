use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

const MAX_ERROR_LOG_ITEMS: usize = 500;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub enum ErrorType {
    ApiError,
    NetworkError,
    FileError,
    AudioError,
    ConfigError,
    UnknownError,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ErrorLogEntry {
    pub id: u64,
    pub timestamp: i64,
    pub error_type: ErrorType,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context: Option<String>,
}

pub struct ErrorLogManager;

impl ErrorLogManager {
    fn get_error_log_path() -> PathBuf {
        let mut path = crate::config::settings::get_app_home_dir();
        path.push("error_log.json");
        path
    }

    /// エラーログを読み込み
    pub fn load_error_log() -> Vec<ErrorLogEntry> {
        let path = Self::get_error_log_path();
        if !path.exists() {
            return Vec::new();
        }

        let content = fs::read_to_string(path).unwrap_or_default();
        serde_json::from_str(&content).unwrap_or_default()
    }

    /// エラーログを保存
    fn save_error_log(entries: &[ErrorLogEntry]) -> Result<(), String> {
        let path = Self::get_error_log_path();
        let content =
            serde_json::to_string_pretty(entries).map_err(|e| format!("JSON作成失敗: {}", e))?;
        fs::write(path, content).map_err(|e| format!("ファイル書き込み失敗: {}", e))?;
        Ok(())
    }

    /// エラーを追加
    pub fn add_error(
        error_type: ErrorType,
        message: &str,
        context: Option<&str>,
    ) -> Result<(), String> {
        let mut entries = Self::load_error_log();

        let now = crate::util::current_unix_timestamp_secs();

        let entry = ErrorLogEntry {
            id: now,
            timestamp: now as i64,
            error_type,
            message: message.to_string(),
            context: context.map(|s| s.to_string()),
        };

        entries.insert(0, entry);

        // 最大件数を超えた場合は古いものを削除
        if entries.len() > MAX_ERROR_LOG_ITEMS {
            entries.truncate(MAX_ERROR_LOG_ITEMS);
        }

        Self::save_error_log(&entries)
    }

    /// エラーログをクリア
    pub fn clear_error_log() -> Result<(), String> {
        Self::save_error_log(&[])
    }
}
