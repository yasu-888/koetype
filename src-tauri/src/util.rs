use std::time::{SystemTime, UNIX_EPOCH};

/// 現在のUNIXタイムスタンプ（秒）を返す
pub fn current_unix_timestamp_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}
