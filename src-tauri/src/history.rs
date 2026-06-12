use chrono::{Duration, Local, NaiveDate, TimeZone};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

const MAX_HISTORY_ITEMS: usize = 1000;
const HISTORY_RETENTION_DAYS: i64 = 30;
const ACTIVITY_RETENTION_DAYS: i64 = 371;

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct HistoryItem {
    pub id: u64,
    pub text: String,
    #[serde(default)]
    pub dictation_text: Option<String>,
    #[serde(default)]
    pub whisper_text: Option<String>,
    #[serde(default)]
    pub stt_provider: Option<String>,
    pub timestamp: i64,
    pub duration_ms: u64,
}

pub struct HistoryManager;

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct StatsSnapshot {
    pub total_entries: usize,
    pub total_characters: usize,
    pub total_words: usize,
    pub total_duration_ms: u64,
    pub avg_wpm: u64,
    pub updated_at: i64,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct DailyUsagePoint {
    pub date: String,
    pub characters: usize,
    pub entries: usize,
}

impl HistoryManager {
    fn remove_file_if_exists(path: &PathBuf, label: &str) -> Result<(), String> {
        if path.exists() {
            fs::remove_file(path).map_err(|e| format!("{}削除エラー: {}", label, e))?;
        }
        Ok(())
    }

    fn get_history_path() -> PathBuf {
        crate::util::app_data_file("history.json")
    }

    fn get_stats_path() -> PathBuf {
        crate::util::app_data_file("stats.json")
    }

    fn get_daily_usage_path() -> PathBuf {
        crate::util::app_data_file("daily_usage.json")
    }

    fn save_stats_snapshot(stats: &StatsSnapshot) -> Result<(), String> {
        crate::util::save_json_pretty(&Self::get_stats_path(), stats, "stats")
    }

    fn load_stats_snapshot_internal() -> StatsSnapshot {
        crate::util::load_json_or_default(&Self::get_stats_path())
    }

    fn update_stats_snapshot_for_entry(text: &str, duration_ms: u64) -> Result<(), String> {
        let mut stats = Self::load_stats_snapshot_internal();
        let added_characters = text.chars().count();
        let added_words = added_characters / 5;

        stats.total_entries = stats.total_entries.saturating_add(1);
        stats.total_characters = stats.total_characters.saturating_add(added_characters);
        stats.total_words = stats.total_words.saturating_add(added_words);
        stats.total_duration_ms = stats.total_duration_ms.saturating_add(duration_ms);
        stats.avg_wpm = if stats.total_duration_ms > 0 {
            ((stats.total_words as f64) / (stats.total_duration_ms as f64 / 60000.0)).round() as u64
        } else {
            0
        };
        stats.updated_at = crate::util::current_unix_timestamp_secs() as i64;

        Self::save_stats_snapshot(&stats)
    }

    fn reset_stats_snapshot() -> Result<(), String> {
        let reset_stats = StatsSnapshot {
            updated_at: crate::util::current_unix_timestamp_secs() as i64,
            ..StatsSnapshot::default()
        };
        Self::save_stats_snapshot(&reset_stats)
    }

    fn save_daily_usage(points: &[DailyUsagePoint]) -> Result<(), String> {
        crate::util::save_json_pretty(&Self::get_daily_usage_path(), points, "daily_usage")
    }

    fn load_daily_usage_internal() -> Vec<DailyUsagePoint> {
        crate::util::load_json_or_default(&Self::get_daily_usage_path())
    }

    fn date_key_from_timestamp(timestamp: i64) -> String {
        Local
            .timestamp_opt(timestamp, 0)
            .single()
            .unwrap_or_else(Local::now)
            .format("%Y-%m-%d")
            .to_string()
    }

    fn prune_daily_usage(points: &mut Vec<DailyUsagePoint>) {
        let today = Local::now().date_naive();
        points.retain(|point| {
            NaiveDate::parse_from_str(&point.date, "%Y-%m-%d")
                .map(|date| (today - date).num_days() < ACTIVITY_RETENTION_DAYS)
                .unwrap_or(false)
        });
        points.sort_by(|a, b| b.date.cmp(&a.date));
    }

    fn update_daily_usage_for_entry(text: &str, timestamp: i64) -> Result<(), String> {
        let mut points = Self::load_daily_usage_internal();
        let date = Self::date_key_from_timestamp(timestamp);
        let characters = text.chars().count();

        if let Some(point) = points.iter_mut().find(|point| point.date == date) {
            point.characters = point.characters.saturating_add(characters);
            point.entries = point.entries.saturating_add(1);
        } else {
            points.push(DailyUsagePoint {
                date,
                characters,
                entries: 1,
            });
        }

        Self::prune_daily_usage(&mut points);
        Self::save_daily_usage(&points)
    }

    fn decrement_daily_usage_for_entry(entry: &HistoryItem) -> Result<(), String> {
        let mut points = Self::load_daily_usage_internal();
        let date = Self::date_key_from_timestamp(entry.timestamp);
        let characters = entry.text.chars().count();

        if let Some(point) = points.iter_mut().find(|point| point.date == date) {
            point.characters = point.characters.saturating_sub(characters);
            point.entries = point.entries.saturating_sub(1);
        }

        points.retain(|point| point.entries > 0 || point.characters > 0);
        Self::prune_daily_usage(&mut points);
        Self::save_daily_usage(&points)
    }

    pub fn get_stats_snapshot() -> StatsSnapshot {
        Self::load_stats_snapshot_internal()
    }

    pub fn get_daily_usage(days: u16) -> Vec<DailyUsagePoint> {
        if days == 0 {
            return Vec::new();
        }

        let mut points = Self::load_daily_usage_internal();
        Self::prune_daily_usage(&mut points);

        let cutoff = Local::now().date_naive() - Duration::days(i64::from(days.saturating_sub(1)));
        points
            .into_iter()
            .filter(|point| {
                NaiveDate::parse_from_str(&point.date, "%Y-%m-%d")
                    .map(|date| date >= cutoff)
                    .unwrap_or(false)
            })
            .collect()
    }

    /// 履歴を読み込む（自動クリーンアップ付き）
    pub fn load_history() -> Vec<HistoryItem> {
        let mut history: Vec<HistoryItem> =
            crate::util::load_json_or_default(&Self::get_history_path());

        // 保持期間（30日）を超えた履歴を自動削除
        let now = crate::util::current_unix_timestamp_secs() as i64;
        let retention_period = HISTORY_RETENTION_DAYS * 24 * 60 * 60;

        let original_len = history.len();
        history.retain(|item| (now - item.timestamp) < retention_period);

        // 変更があった場合のみ保存
        if history.len() != original_len {
            let _ = Self::save_history(&history);
        }

        history
    }

    /// 履歴を保存
    fn save_history(history: &[HistoryItem]) -> Result<(), String> {
        crate::util::save_json_pretty(&Self::get_history_path(), history, "履歴")
    }

    /// LLMテキストとOS標準テキストを同時に保存する新しい関数
    pub fn add_entry_with_dictation(
        text: &str,
        dictation_text: Option<&str>,
        whisper_text: Option<&str>,
        stt_provider: Option<&str>,
        duration_ms: u64,
    ) -> Result<HistoryItem, String> {
        let mut history = Self::load_history();

        let timestamp = crate::util::current_unix_timestamp_secs() as i64;

        let item = HistoryItem {
            id: timestamp as u64,
            text: text.to_string(),
            dictation_text: dictation_text.map(|s| s.to_string()),
            whisper_text: whisper_text.map(|s| s.to_string()),
            stt_provider: stt_provider.map(|s| s.to_string()),
            timestamp,
            duration_ms,
        };

        history.insert(0, item.clone());

        // 最大件数を超えたら古いものを削除
        if history.len() > MAX_HISTORY_ITEMS {
            history.truncate(MAX_HISTORY_ITEMS);
        }

        Self::save_history(&history)?;
        Self::update_daily_usage_for_entry(text, timestamp)?;
        Self::update_stats_snapshot_for_entry(text, duration_ms)?;

        Ok(item)
    }

    /// 最後の文字起こしを取得
    pub fn get_last_transcription() -> Option<HistoryItem> {
        let history = Self::load_history();
        history
            .iter()
            .find(|item| !item.text.trim().is_empty())
            .cloned()
    }

    /// 最新のOS標準音声入力を取得
    pub fn get_last_dictation_text() -> Option<String> {
        let history = Self::load_history();
        history
            .iter()
            .find_map(|item| {
                item.dictation_text
                    .as_ref()
                    .map(|text| text.trim().to_string())
            })
            .filter(|text| !text.is_empty())
    }

    /// 最新のWhisper文字起こしを取得
    pub fn get_last_whisper_text() -> Option<String> {
        let history = Self::load_history();
        history
            .iter()
            .find_map(|item| {
                item.whisper_text
                    .as_ref()
                    .map(|text| text.trim().to_string())
            })
            .filter(|text| !text.is_empty())
    }

    /// 履歴をクリア
    pub fn clear_history() -> Result<(), String> {
        let history_path = Self::get_history_path();
        let daily_usage_path = Self::get_daily_usage_path();
        Self::remove_file_if_exists(&history_path, "履歴ファイル")?;
        Self::remove_file_if_exists(&daily_usage_path, "日次集計ファイル")?;
        Self::reset_stats_snapshot()?;
        Ok(())
    }

    /// 特定の履歴を削除
    pub fn remove_entry(id: u64) -> Result<(), String> {
        let mut history = Self::load_history();
        let removed_item = history.iter().find(|item| item.id == id).cloned();
        history.retain(|item| item.id != id);
        Self::save_history(&history)?;
        if let Some(item) = removed_item {
            Self::decrement_daily_usage_for_entry(&item)?;
        }
        Ok(())
    }
}
