use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::process::Command;

#[cfg(target_os = "windows")]
const CREATE_NO_WINDOW: u32 = 0x08000000;
const WHISPER_MODEL_BASE_URL: &str = "https://huggingface.co/ggerganov/whisper.cpp/resolve/main";

#[derive(serde::Serialize, Clone, Debug)]
pub struct WhisperInstallProgress {
    pub phase: String,
    pub downloaded_bytes: u64,
    pub total_bytes: Option<u64>,
    pub percent: Option<u8>,
    pub message: String,
}

fn looks_like_path(input: &str) -> bool {
    Path::new(input).is_absolute() || input.contains('/') || input.contains('\\')
}

fn is_executable_file(path: &Path) -> bool {
    if !path.is_file() {
        return false;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let Ok(metadata) = std::fs::metadata(path) else {
            return false;
        };
        metadata.permissions().mode() & 0o111 != 0
    }
    #[cfg(not(unix))]
    {
        true
    }
}

fn is_valid_model_file(path: &Path) -> bool {
    if !path.is_file() {
        return false;
    }
    let Ok(metadata) = std::fs::metadata(path) else {
        return false;
    };
    metadata.len() >= 1_000_000
}

fn find_in_path(command: &str) -> Option<PathBuf> {
    let path_var = std::env::var_os("PATH")?;
    #[cfg(target_os = "windows")]
    {
        let cmd = Path::new(command);
        let has_ext = cmd.extension().is_some();
        let pathext =
            std::env::var("PATHEXT").unwrap_or_else(|_| ".COM;.EXE;.BAT;.CMD".to_string());
        let exts: Vec<String> = if has_ext {
            vec!["".to_string()]
        } else {
            pathext
                .split(';')
                .map(str::trim)
                .filter(|e| !e.is_empty())
                .map(|e| e.to_ascii_lowercase())
                .collect()
        };
        for dir in std::env::split_paths(&path_var) {
            if has_ext {
                let candidate = dir.join(command);
                if is_executable_file(&candidate) {
                    return Some(candidate);
                }
                continue;
            }
            for ext in &exts {
                let with_ext = format!("{command}{ext}");
                let candidate = dir.join(with_ext);
                if is_executable_file(&candidate) {
                    return Some(candidate);
                }
            }
        }
    }
    #[cfg(not(target_os = "windows"))]
    {
        for dir in std::env::split_paths(&path_var) {
            let candidate = dir.join(command);
            if is_executable_file(&candidate) {
                return Some(candidate);
            }
        }
    }
    None
}

fn resolve_whisper_cli_path() -> Result<String, String> {
    let mut candidates: Vec<String> = Vec::new();

    if let Ok(path) = std::env::var("KOETYPE_WHISPER_CLI_PATH") {
        let trimmed = path.trim();
        if !trimmed.is_empty() {
            candidates.push(trimmed.to_string());
        }
    }
    if let Some(path) = crate::config::settings::get_whisper_cli_path() {
        candidates.push(path);
    }

    let app_home = crate::config::settings::get_app_home_dir();
    if cfg!(target_os = "windows") {
        candidates.push(
            app_home
                .join("bin")
                .join("whisper-cli.exe")
                .to_string_lossy()
                .to_string(),
        );
        candidates.push(
            app_home
                .join("bin")
                .join("whisper-cli")
                .to_string_lossy()
                .to_string(),
        );
        candidates.push("whisper-cli.exe".to_string());
        candidates.push("whisper-cli".to_string());
    } else {
        if let Some(path) = bundled_whisper_cli_path() {
            candidates.push(path.to_string_lossy().to_string());
        }
        candidates.push(
            app_home
                .join("bin")
                .join("whisper-cli")
                .to_string_lossy()
                .to_string(),
        );
        candidates.push("/opt/homebrew/bin/whisper-cli".to_string());
        candidates.push("/usr/local/bin/whisper-cli".to_string());
        candidates.push("whisper-cli".to_string());
    }

    let mut checked: Vec<String> = Vec::new();
    for candidate in candidates {
        if candidate.trim().is_empty() {
            continue;
        }
        if looks_like_path(&candidate) {
            checked.push(candidate.clone());
            let path = Path::new(&candidate);
            if is_executable_file(path) {
                return Ok(candidate);
            }
            continue;
        }
        checked.push(format!("PATH:{candidate}"));
        if let Some(found) = find_in_path(&candidate) {
            return Ok(found.to_string_lossy().to_string());
        }
    }

    Err(format!(
        "whisper-cli が見つかりません。確認した候補: {}。\
必要なら config.toml の whisper_cli_path または KOETYPE_WHISPER_CLI_PATH を設定してください。",
        checked.join(", ")
    ))
}

fn koetype_home_dir() -> Result<PathBuf, String> {
    if let Ok(path) = std::env::var("KOETYPE_HOME") {
        return Ok(PathBuf::from(path));
    }
    Ok(crate::config::settings::get_app_home_dir())
}

pub fn default_model_name_for_platform() -> &'static str {
    if cfg!(target_os = "windows") {
        "base"
    } else {
        "large-v3-turbo"
    }
}

fn preferred_model_names_for_platform() -> &'static [&'static str] {
    if cfg!(target_os = "windows") {
        &["base", "large-v3-turbo", "large-v3"]
    } else {
        &["large-v3-turbo", "large-v3", "base"]
    }
}

fn model_name_to_filename(model_name: &str) -> &'static str {
    match model_name {
        "base" => "ggml-base-q5_1.bin",
        "large-v3" => "ggml-large-v3-q5_0.bin",
        "large-v3-turbo" => "ggml-large-v3-turbo-q5_0.bin",
        _ => "ggml-large-v3-turbo-q5_0.bin",
    }
}

fn candidate_model_paths_from_directory(model_dir: &Path) -> Vec<PathBuf> {
    let mut candidates: Vec<PathBuf> = Vec::new();
    for model_name in preferred_model_names_for_platform() {
        candidates.push(model_dir.join(model_name_to_filename(model_name)));
    }

    let Ok(entries) = std::fs::read_dir(model_dir) else {
        return candidates;
    };
    let mut discovered: Vec<PathBuf> = entries
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| {
            path.is_file()
                && path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .map(|name| name.starts_with("ggml-") && name.ends_with(".bin"))
                    .unwrap_or(false)
        })
        .collect();
    discovered.sort_by(|a, b| a.file_name().cmp(&b.file_name()));

    for path in discovered {
        if !candidates.contains(&path) {
            candidates.push(path);
        }
    }

    candidates
}

fn download_whisper_model_to(
    model_name: &str,
    model_path: &Path,
    on_progress: &mut dyn FnMut(WhisperInstallProgress),
) -> Result<(), String> {
    use std::io::{Read, Write};

    let filename = model_name_to_filename(model_name);
    let url = format!("{WHISPER_MODEL_BASE_URL}/{filename}");
    let tmp_path = model_path.with_extension("bin.tmp");

    if let Some(parent) = model_path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| {
            format!(
                "Whisperモデル用ディレクトリ作成に失敗: {} ({})",
                e,
                parent.display()
            )
        })?;
    }

    let mut response = reqwest::blocking::get(url.as_str())
        .and_then(|r| r.error_for_status())
        .map_err(|e| format!("Whisperモデルのダウンロードに失敗: {}", e))?;
    let total_bytes = response.content_length();
    on_progress(WhisperInstallProgress {
        phase: "downloading".to_string(),
        downloaded_bytes: 0,
        total_bytes,
        percent: total_bytes.map(|_| 0),
        message: if total_bytes.is_some() {
            "Whisperモデルをダウンロード中 (0%)".to_string()
        } else {
            "Whisperモデルをダウンロード中 (0 bytes)".to_string()
        },
    });

    let mut file = std::fs::File::create(&tmp_path).map_err(|e| {
        format!(
            "Whisperモデル一時ファイルの作成に失敗: {} ({})",
            e,
            tmp_path.display()
        )
    })?;
    let mut downloaded_bytes: u64 = 0;
    let mut last_percent: Option<u8> = None;
    let mut last_reported_bytes: u64 = 0;
    const UNKNOWN_TOTAL_REPORT_STEP_BYTES: u64 = 4 * 1024 * 1024;
    let mut buffer = [0u8; 64 * 1024];
    loop {
        let read_size = response.read(&mut buffer).map_err(|e| {
            format!(
                "Whisperモデルのダウンロード中に読み込み失敗: {} ({})",
                e, url
            )
        })?;
        if read_size == 0 {
            break;
        }
        file.write_all(&buffer[..read_size]).map_err(|e| {
            format!(
                "Whisperモデル一時ファイルへの書き込みに失敗: {} ({})",
                e,
                tmp_path.display()
            )
        })?;
        downloaded_bytes += read_size as u64;
        let percent = total_bytes.map(|total| {
            let ratio = (downloaded_bytes as f64 / total as f64) * 100.0;
            ratio.clamp(0.0, 100.0).round() as u8
        });
        match percent {
            Some(p) => {
                if Some(p) != last_percent {
                    last_percent = Some(p);
                    on_progress(WhisperInstallProgress {
                        phase: "downloading".to_string(),
                        downloaded_bytes,
                        total_bytes,
                        percent: Some(p),
                        message: format!("Whisperモデルをダウンロード中 ({}%)", p),
                    });
                }
            }
            None => {
                if downloaded_bytes.saturating_sub(last_reported_bytes)
                    >= UNKNOWN_TOTAL_REPORT_STEP_BYTES
                {
                    last_reported_bytes = downloaded_bytes;
                    on_progress(WhisperInstallProgress {
                        phase: "downloading".to_string(),
                        downloaded_bytes,
                        total_bytes,
                        percent: None,
                        message: format!(
                            "Whisperモデルをダウンロード中 ({} bytes)",
                            downloaded_bytes
                        ),
                    });
                }
            }
        }
    }

    if total_bytes.is_none() && downloaded_bytes != last_reported_bytes {
        on_progress(WhisperInstallProgress {
            phase: "downloading".to_string(),
            downloaded_bytes,
            total_bytes,
            percent: None,
            message: format!("Whisperモデルをダウンロード中 ({} bytes)", downloaded_bytes),
        });
    }

    file.flush().map_err(|e| {
        format!(
            "Whisperモデル一時ファイルの flush に失敗: {} ({})",
            e,
            tmp_path.display()
        )
    })?;

    if !is_valid_model_file(&tmp_path) {
        let _ = std::fs::remove_file(&tmp_path);
        return Err(format!(
            "Whisperモデルのダウンロード結果が不正です: {}",
            tmp_path.display()
        ));
    }

    on_progress(WhisperInstallProgress {
        phase: "moving".to_string(),
        downloaded_bytes,
        total_bytes,
        percent: Some(100),
        message: "Whisperモデルを配置中".to_string(),
    });

    std::fs::rename(&tmp_path, model_path).map_err(|e| {
        format!(
            "Whisperモデルの配置に失敗: {} ({} -> {})",
            e,
            tmp_path.display(),
            model_path.display()
        )
    })?;
    Ok(())
}

fn default_whisper_model_path_with_progress(
    model_name: &str,
    allow_auto_download: bool,
    mut on_progress: Option<&mut dyn FnMut(WhisperInstallProgress)>,
) -> Result<String, String> {
    if let Ok(path) = std::env::var("KOETYPE_WHISPER_MODEL_PATH") {
        return Ok(path);
    }
    if let Some(path) = crate::config::settings::get_whisper_model_path() {
        let configured = PathBuf::from(&path);
        if is_valid_model_file(&configured) {
            return Ok(path);
        }
    }

    let filename = model_name_to_filename(model_name);
    if let Some(path) = bundled_whisper_model_path(filename) {
        if is_valid_model_file(&path) {
            return Ok(path.to_string_lossy().to_string());
        }
    }
    let app_home = koetype_home_dir()?;
    let model_path = app_home.join(format!("models/whisper/{}", filename));

    if is_valid_model_file(&model_path) {
        return Ok(model_path.to_string_lossy().to_string());
    }

    if allow_auto_download {
        if let Some(callback) = on_progress.as_mut() {
            callback(WhisperInstallProgress {
                phase: "preparing".to_string(),
                downloaded_bytes: 0,
                total_bytes: None,
                percent: Some(0),
                message: "Whisperモデルのダウンロードを準備中".to_string(),
            });
            download_whisper_model_to(model_name, &model_path, *callback)?;
        } else {
            let mut noop = |_progress: WhisperInstallProgress| {};
            download_whisper_model_to(model_name, &model_path, &mut noop)?;
        }
        if is_valid_model_file(&model_path) {
            return Ok(model_path.to_string_lossy().to_string());
        }
    }

    Err(format!(
        "Whisperモデルが見つかりません: {}\ndocs/whisper-model-setup.md の手順でモデルを配置してください。",
        model_path.display()
    ))
}

fn resolve_preferred_whisper_model_path(allow_auto_download: bool) -> Result<String, String> {
    if let Ok(path) = std::env::var("KOETYPE_WHISPER_MODEL_PATH") {
        let trimmed = path.trim();
        if !trimmed.is_empty() && is_valid_model_file(Path::new(trimmed)) {
            return Ok(trimmed.to_string());
        }
    }

    if let Some(path) = crate::config::settings::get_whisper_model_path() {
        let configured = PathBuf::from(&path);
        if is_valid_model_file(&configured) {
            return Ok(path);
        }
    }

    for model_name in preferred_model_names_for_platform() {
        let filename = model_name_to_filename(model_name);
        if let Some(path) = bundled_whisper_model_path(filename) {
            if is_valid_model_file(&path) {
                return Ok(path.to_string_lossy().to_string());
            }
        }
    }

    let app_home = koetype_home_dir()?;
    let model_dir = app_home.join("models/whisper");
    for candidate in candidate_model_paths_from_directory(&model_dir) {
        if is_valid_model_file(&candidate) {
            return Ok(candidate.to_string_lossy().to_string());
        }
    }

    if allow_auto_download {
        let default_model_name = default_model_name_for_platform();
        return default_whisper_model_path(default_model_name, true);
    }

    Err(format!(
        "Whisperモデルが見つかりません: {}/ggml-*.bin\ndocs/whisper-model-setup.md の手順でモデルを配置してください。",
        model_dir.display()
    ))
}

fn default_whisper_model_path(
    model_name: &str,
    allow_auto_download: bool,
) -> Result<String, String> {
    default_whisper_model_path_with_progress(model_name, allow_auto_download, None)
}

pub fn install_whisper_model_with_progress(
    model_name: &str,
    on_progress: &mut dyn FnMut(WhisperInstallProgress),
) -> Result<String, String> {
    let model_path = default_whisper_model_path_with_progress(model_name, true, Some(on_progress))?;
    on_progress(WhisperInstallProgress {
        phase: "completed".to_string(),
        downloaded_bytes: 0,
        total_bytes: None,
        percent: Some(100),
        message: "Whisperモデルのインストールが完了しました".to_string(),
    });
    Ok(model_path)
}

#[cfg(target_os = "macos")]
fn bundled_resource_root_from_current_exe() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    let macos_dir = exe.parent()?;
    let contents_dir = macos_dir.parent()?;
    Some(contents_dir.join("Resources"))
}

#[cfg(not(target_os = "macos"))]
fn bundled_resource_root_from_current_exe() -> Option<PathBuf> {
    None
}

fn bundled_whisper_cli_path() -> Option<PathBuf> {
    let resource_dir = bundled_resource_root_from_current_exe()?;
    Some(resource_dir.join("bin").join("whisper-cli"))
}

fn bundled_whisper_model_path(filename: &str) -> Option<PathBuf> {
    let resource_dir = bundled_resource_root_from_current_exe()?;
    Some(resource_dir.join("models").join("whisper").join(filename))
}

#[derive(serde::Serialize, Clone, Debug)]
pub struct WhisperRuntimeStatus {
    pub cli_path: Option<String>,
    pub model_path: Option<String>,
    pub cli_bundled: bool,
    pub model_bundled: bool,
}

pub fn inspect_whisper_runtime_status_auto() -> WhisperRuntimeStatus {
    let cli_path = resolve_whisper_cli_path().ok();
    let model_path = resolve_preferred_whisper_model_path(false)
        .ok()
        .filter(|p| is_valid_model_file(Path::new(p)));

    let bundled_root = bundled_resource_root_from_current_exe();
    let bundled_cli_root = bundled_root.as_ref().map(|root| root.join("bin"));
    let bundled_model_root = bundled_root
        .as_ref()
        .map(|root| root.join("models/whisper"));

    let cli_bundled = bundled_cli_root
        .as_ref()
        .zip(cli_path.as_ref())
        .map(|(root, resolved)| Path::new(resolved).starts_with(root))
        .unwrap_or(false);
    let model_bundled = bundled_model_root
        .as_ref()
        .zip(model_path.as_ref())
        .map(|(root, resolved)| Path::new(resolved).starts_with(root))
        .unwrap_or(false);

    WhisperRuntimeStatus {
        cli_path,
        model_path,
        cli_bundled,
        model_bundled,
    }
}

pub fn inspect_whisper_runtime_status(model_name: &str) -> WhisperRuntimeStatus {
    let filename = model_name_to_filename(model_name);
    let bundled_cli = bundled_whisper_cli_path().filter(|p| p.exists());
    let bundled_model = bundled_whisper_model_path(filename).filter(|p| is_valid_model_file(p));

    let cli_path = resolve_whisper_cli_path().ok();
    let model_path = default_whisper_model_path(model_name, false)
        .ok()
        .filter(|p| is_valid_model_file(Path::new(p)));
    let cli_bundled = bundled_cli
        .as_ref()
        .and_then(|p| p.to_str())
        .zip(cli_path.as_deref())
        .map(|(bundled, resolved)| bundled == resolved)
        .unwrap_or(false);
    let model_bundled = bundled_model
        .as_ref()
        .and_then(|p| p.to_str())
        .zip(model_path.as_deref())
        .map(|(bundled, resolved)| bundled == resolved)
        .unwrap_or(false);

    WhisperRuntimeStatus {
        cli_path,
        model_path,
        cli_bundled,
        model_bundled,
    }
}

fn prepare_output_base(audio_file: &str) -> Result<String, String> {
    if let Ok(path) = std::env::var("KOETYPE_WHISPER_OUTPUT_BASE") {
        return Ok(path);
    }

    let app_home = koetype_home_dir()?;
    let output_dir = if let Ok(path) = std::env::var("KOETYPE_WHISPER_OUTPUT_DIR") {
        PathBuf::from(path)
    } else if let Some(path) = crate::config::settings::get_whisper_output_dir() {
        PathBuf::from(path)
    } else {
        app_home.join("tmp/whisper")
    };
    std::fs::create_dir_all(&output_dir).map_err(|e| {
        format!(
            "Whisper出力ディレクトリ作成に失敗: {} ({})",
            e,
            output_dir.to_string_lossy()
        )
    })?;

    let stem = Path::new(audio_file)
        .file_stem()
        .and_then(|s| s.to_str())
        .filter(|s| !s.is_empty())
        .unwrap_or("recording");

    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);

    Ok(output_dir
        .join(format!("{}_{}", stem, ts))
        .to_string_lossy()
        .to_string())
}

fn output_txt_path(output_base: &str) -> String {
    format!("{}.txt", output_base)
}

fn transcripts_jsonl_path() -> PathBuf {
    if let Ok(path) = std::env::var("KOETYPE_TRANSCRIPTS_PATH") {
        let p = PathBuf::from(path);
        if let Some(parent) = p.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        return p;
    }
    if let Some(path) = crate::config::settings::get_whisper_transcripts_path() {
        let p = PathBuf::from(path);
        if let Some(parent) = p.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        return p;
    }
    crate::config::settings::get_app_home_dir().join("transcripts.jsonl")
}

fn append_transcript_jsonl(
    audio_file: &str,
    language: &str,
    model_path: &str,
    text: &str,
) -> Result<(), String> {
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let row = serde_json::json!({
        "timestamp": ts,
        "provider": "whisper-local",
        "audio_file": audio_file,
        "language": language,
        "model_path": model_path,
        "text": text,
    });
    let path = transcripts_jsonl_path();
    use std::io::Write;
    let mut f = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .map_err(|e| {
            format!(
                "transcripts.jsonl 書き込み失敗: {} ({})",
                e,
                path.to_string_lossy()
            )
        })?;
    f.write_all(row.to_string().as_bytes())
        .and_then(|_| f.write_all(b"\n"))
        .map_err(|e| {
            format!(
                "transcripts.jsonl 書き込み失敗: {} ({})",
                e,
                path.to_string_lossy()
            )
        })?;
    Ok(())
}

fn append_whisper_log(stdout: &[u8], stderr: &[u8]) -> Result<(), String> {
    let app_home = koetype_home_dir()?;
    let log_dir = if let Ok(path) = std::env::var("KOETYPE_LOG_DIR") {
        PathBuf::from(path)
    } else if let Some(path) = crate::config::settings::get_whisper_log_dir() {
        PathBuf::from(path)
    } else {
        app_home.join("logs")
    };
    std::fs::create_dir_all(&log_dir).map_err(|e| {
        format!(
            "ログディレクトリ作成に失敗: {} ({})",
            e,
            log_dir.to_string_lossy()
        )
    })?;
    let log_path = log_dir.join("whisper-cli.log");
    let mut content = String::new();
    content.push_str("-----\n");
    content.push_str(&String::from_utf8_lossy(stdout));
    content.push_str("\n");
    content.push_str(&String::from_utf8_lossy(stderr));
    content.push_str("\n");
    use std::io::Write;
    let mut f = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_path)
        .map_err(|e| {
            format!(
                "ログファイル書き込みに失敗: {} ({})",
                e,
                log_path.to_string_lossy()
            )
        })?;
    f.write_all(content.as_bytes()).map_err(|e| {
        format!(
            "ログファイル書き込みに失敗: {} ({})",
            e,
            log_path.to_string_lossy()
        )
    })?;
    Ok(())
}

pub async fn transcribe_audio_local(
    audio_file: &str,
    language: &str,
    model_name: &str,
) -> Result<String, String> {
    let cli_path = resolve_whisper_cli_path()?;
    let model_path = if model_name == "auto" {
        resolve_preferred_whisper_model_path(true)?
    } else {
        default_whisper_model_path(model_name, true)?
    };
    let output_base = prepare_output_base(audio_file)?;

    if !Path::new(audio_file).exists() {
        return Err(format!("音声ファイルが見つかりません: {}", audio_file));
    }
    if !Path::new(&model_path).exists() {
        return Err(format!("Whisperモデルが見つかりません: {}", model_path));
    }

    let threads = std::thread::available_parallelism()
        .map(|v| v.get().to_string())
        .unwrap_or_else(|_| "8".to_string());

    let mut command = Command::new(&cli_path);
    command
        .arg("-m")
        .arg(&model_path)
        .arg("-f")
        .arg(audio_file)
        .arg("-l")
        .arg(language)
        .arg("-t")
        .arg(threads)
        .arg("-otxt")
        .arg("-of")
        .arg(output_base.as_str());

    #[cfg(target_os = "windows")]
    {
        // whisper-cli 実行時にコンソールウィンドウを新規表示しない。
        command.creation_flags(CREATE_NO_WINDOW);
    }

    let output = command
        .output()
        .await
        .map_err(|e| format!("whisper-cli 実行失敗: {}", e))?;

    let _ = append_whisper_log(&output.stdout, &output.stderr);

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
        let details = if !stderr.is_empty() { stderr } else { stdout };
        return Err(format!("whisper-cli 実行エラー: {}", details));
    }

    let txt_path = output_txt_path(output_base.as_str());
    let text = tokio::fs::read_to_string(&txt_path)
        .await
        .map_err(|e| format!("whisper出力読み込み失敗 ({}): {}", txt_path, e))?;
    let normalized = text.trim().to_string();
    if normalized.is_empty() {
        return Err("whisper の文字起こし結果が空です".to_string());
    }

    let _ = append_transcript_jsonl(audio_file, language, &model_path, &normalized);
    let _ = std::fs::remove_file(&txt_path);

    Ok(normalized)
}
