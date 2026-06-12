use serde::de::DeserializeOwned;
use serde::Serialize;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

/// 現在のUNIXタイムスタンプ（秒）を返す
pub fn current_unix_timestamp_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// アプリホームディレクトリ内のデータファイルパスを返す
pub fn app_data_file(name: &str) -> PathBuf {
    crate::config::settings::get_app_home_dir().join(name)
}

/// JSON ファイルを読み込む。ファイルが無い・読めない・パースできない場合は
/// Default にフォールバックする。常駐アプリのため、壊れたデータファイルで
/// クラッシュ・起動不能にしないことを優先している（データは履歴・統計などの
/// 再生成可能な情報のみで、失っても致命的ではない）。
pub fn load_json_or_default<T: DeserializeOwned + Default>(path: &Path) -> T {
    if !path.exists() {
        return T::default();
    }
    let content = fs::read_to_string(path).unwrap_or_default();
    serde_json::from_str(&content).unwrap_or_default()
}

/// JSON ファイルへ pretty 形式で保存する。label はエラーメッセージの主語。
pub fn save_json_pretty<T: Serialize + ?Sized>(
    path: &Path,
    value: &T,
    label: &str,
) -> Result<(), String> {
    let content = serde_json::to_string_pretty(value)
        .map_err(|e| format!("{} JSON変換エラー: {}", label, e))?;
    fs::write(path, content).map_err(|e| format!("{} ファイル書き込みエラー: {}", label, e))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_json_path(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!("koetype-util-test-{}-{}", std::process::id(), name))
    }

    #[test]
    fn load_json_or_default_returns_default_when_missing() {
        let path = temp_json_path("missing.json");
        let value: Vec<String> = load_json_or_default(&path);
        assert!(value.is_empty());
    }

    #[test]
    fn load_json_or_default_returns_default_on_broken_json() {
        let path = temp_json_path("broken.json");
        fs::write(&path, "{ not json").unwrap();
        let value: Vec<String> = load_json_or_default(&path);
        assert!(value.is_empty());
        let _ = fs::remove_file(&path);
    }

    #[test]
    fn save_then_load_roundtrips() {
        let path = temp_json_path("roundtrip.json");
        let original = vec!["a".to_string(), "b".to_string()];
        save_json_pretty(&path, &original, "テスト").unwrap();
        let loaded: Vec<String> = load_json_or_default(&path);
        assert_eq!(loaded, original);
        let _ = fs::remove_file(&path);
    }
}
