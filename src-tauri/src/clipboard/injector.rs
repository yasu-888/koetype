use enigo::{Direction, Enigo, Key, Keyboard, Settings};
use serde::Serialize;
#[cfg(target_os = "macos")]
use std::ffi::c_void;
use std::thread;
use std::time::Duration;
use thiserror::Error;
use tracing::{debug, info, warn};

#[derive(Error, Debug)]
pub enum PasteError {
    #[error("クリップボードの設定に失敗しました: {0}")]
    ClipboardSet(String),

    #[error("ペースト注入に失敗しました: {0}")]
    PasteInjection(String),

    #[cfg(target_os = "macos")]
    #[error("権限不足のためペースト注入に失敗しました: {0}")]
    PermissionDenied(String),
}

#[derive(Error, Debug)]
pub enum CopyError {
    #[error("コピーショートカットの実行に失敗しました: {0}")]
    Shortcut(String),
}

#[derive(Serialize, Clone, Debug)]
pub struct AutomationProbeResult {
    pub ok: bool,
    pub status_code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
}

#[derive(Serialize, Clone, Debug)]
pub struct EnigoProbeResult {
    pub ok: bool,
    pub message: String,
}

#[derive(Serialize, Clone, Debug, Default)]
pub struct PasteMethodDiagnostics {
    pub applescript_status_code: Option<i32>,
    pub applescript_stderr: Option<String>,
    pub enigo_error: Option<String>,
    pub used_fallback: bool,
}

#[derive(Serialize, Clone, Debug)]
pub struct PasteExecutionResult {
    pub method: String,
    pub diagnostics: PasteMethodDiagnostics,
}

#[cfg(target_os = "macos")]
#[derive(Serialize, Clone, Debug, Default)]
pub struct TypeMethodDiagnostics {
    pub enigo_error: Option<String>,
}

#[cfg(target_os = "macos")]
#[derive(Serialize, Clone, Debug)]
pub struct TypeExecutionResult {
    pub method: String,
    pub diagnostics: TypeMethodDiagnostics,
}

#[cfg(target_os = "macos")]
#[link(name = "ApplicationServices", kind = "framework")]
unsafe extern "C" {
    fn AXIsProcessTrusted() -> bool;
    fn AXIsProcessTrustedWithOptions(the_dict: *const c_void) -> bool;
    static kAXTrustedCheckOptionPrompt: *const c_void;
}

#[cfg(target_os = "macos")]
#[link(name = "CoreFoundation", kind = "framework")]
unsafe extern "C" {
    fn CFDictionaryCreate(
        allocator: *const c_void,
        keys: *const *const c_void,
        values: *const *const c_void,
        num_values: isize,
        key_callbacks: *const c_void,
        value_callbacks: *const c_void,
    ) -> *const c_void;
    static kCFBooleanTrue: *const c_void;
    fn CFRelease(cf: *const c_void);
}

#[cfg(target_os = "macos")]
pub fn is_macos_accessibility_trusted() -> bool {
    unsafe { AXIsProcessTrusted() }
}

#[cfg(target_os = "macos")]
pub fn request_macos_accessibility_prompt() -> bool {
    unsafe {
        let key = kAXTrustedCheckOptionPrompt;
        let value = kCFBooleanTrue;
        let keys = [key];
        let values = [value];
        let options = CFDictionaryCreate(
            std::ptr::null(),
            keys.as_ptr(),
            values.as_ptr(),
            1,
            std::ptr::null(),
            std::ptr::null(),
        );

        if options.is_null() {
            return AXIsProcessTrusted();
        }

        let trusted = AXIsProcessTrustedWithOptions(options);
        CFRelease(options);
        trusted
    }
}

#[cfg(not(target_os = "macos"))]
pub fn is_macos_accessibility_trusted() -> bool {
    false
}

#[cfg(target_os = "macos")]
pub fn run_applescript_probe() -> AutomationProbeResult {
    let script =
        "tell application \"System Events\" to get name of first process whose frontmost is true";
    match std::process::Command::new("osascript")
        .arg("-e")
        .arg(script)
        .output()
    {
        Ok(output) => AutomationProbeResult {
            ok: output.status.success(),
            status_code: output.status.code(),
            stdout: String::from_utf8_lossy(&output.stdout).trim().to_string(),
            stderr: String::from_utf8_lossy(&output.stderr).trim().to_string(),
        },
        Err(e) => AutomationProbeResult {
            ok: false,
            status_code: None,
            stdout: String::new(),
            stderr: e.to_string(),
        },
    }
}

#[cfg(not(target_os = "macos"))]
pub fn run_applescript_probe() -> AutomationProbeResult {
    AutomationProbeResult {
        ok: false,
        status_code: None,
        stdout: String::new(),
        stderr: "not supported on this platform".to_string(),
    }
}

#[cfg(target_os = "macos")]
pub fn get_frontmost_app_bundle_id() -> Option<String> {
    let script = "tell application \"System Events\" to get bundle identifier of first application process whose frontmost is true";
    let output = std::process::Command::new("osascript")
        .arg("-e")
        .arg(script)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let s = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if s.is_empty() {
        None
    } else {
        Some(s)
    }
}

#[cfg(not(target_os = "macos"))]
pub fn get_frontmost_app_bundle_id() -> Option<String> {
    None
}

#[cfg(target_os = "macos")]
pub fn run_enigo_probe() -> EnigoProbeResult {
    match std::panic::catch_unwind(|| Enigo::new(&Settings::default())) {
        Ok(Ok(mut enigo)) => {
            // Shift の押下/解放だけを試して、極力副作用を抑えつつイベント注入経路を確認する。
            if let Err(e) = enigo.key(Key::Shift, Direction::Press) {
                return EnigoProbeResult {
                    ok: false,
                    message: format!("Shift press failed: {}", e),
                };
            }
            thread::sleep(Duration::from_millis(10));
            if let Err(e) = enigo.key(Key::Shift, Direction::Release) {
                return EnigoProbeResult {
                    ok: false,
                    message: format!("Shift release failed: {}", e),
                };
            }
            EnigoProbeResult {
                ok: true,
                message: "enigo key injection path returned Ok".to_string(),
            }
        }
        Ok(Err(e)) => EnigoProbeResult {
            ok: false,
            message: format!("Enigo init failed: {}", e),
        },
        Err(_) => EnigoProbeResult {
            ok: false,
            message: "Enigo init panicked".to_string(),
        },
    }
}

#[cfg(not(target_os = "macos"))]
pub fn run_enigo_probe() -> EnigoProbeResult {
    EnigoProbeResult {
        ok: false,
        message: "not supported on this platform".to_string(),
    }
}

/// テキストをクリップボードに設定し、アクティブアプリにペースト注入
pub fn paste_text_to_active_app(text: &str) -> Result<PasteExecutionResult, PasteError> {
    info!("ペースト注入開始: text_len={}", text.len());
    debug!("ペースト注入を実行: {} 文字", text.len());

    // macOS では呼び出し元（lib.rs）で Clipboard API 経由で先に設定する。
    #[cfg(target_os = "macos")]
    {
        if text.is_empty() {
            return Err(PasteError::ClipboardSet(
                "空文字のためペースト対象がありません".to_string(),
            ));
        }
    }

    #[cfg(target_os = "windows")]
    {
        use arboard::Clipboard;
        let mut clipboard = Clipboard::new().map_err(|e| {
            PasteError::ClipboardSet(format!("クリップボードの初期化に失敗: {}", e))
        })?;
        clipboard.set_text(text).map_err(|e| {
            PasteError::ClipboardSet(format!("クリップボードへの書き込みに失敗: {}", e))
        })?;
    }

    debug!("クリップボード設定完了");

    // 短い待機時間（ペーストの準備）
    thread::sleep(Duration::from_millis(100));

    // Cmd+V をシミュレート
    debug!("ペースト注入を実行");

    #[cfg(target_os = "macos")]
    {
        let ax_trusted = is_macos_accessibility_trusted();
        let mut diagnostics = PasteMethodDiagnostics::default();
        // 本番 macOS では、Enigo が成功扱いでも実際に Cmd+V が反映されないケースがあるため
        // まず AppleScript を優先し、失敗時のみ Enigo にフォールバックする。
        info!("ペースト経路: AppleScript 実行開始");
        match std::process::Command::new("osascript")
            .arg("-e")
            .arg("tell application \"System Events\" to key code 9 using {command down}")
            .output()
        {
            Ok(output) if output.status.success() => {
                diagnostics.applescript_status_code = output.status.code();
                info!("ペースト注入完了: method=applescript");
                return Ok(PasteExecutionResult {
                    method: "applescript".to_string(),
                    diagnostics,
                });
            }
            Ok(output) => {
                let stderr = String::from_utf8_lossy(&output.stderr).to_string();
                diagnostics.applescript_status_code = output.status.code();
                diagnostics.applescript_stderr = Some(stderr.clone());
                warn!(
                    "AppleScript失敗: status={:?} stderr={:?}",
                    output.status.code(),
                    stderr
                );
            }
            Err(e) => {
                warn!("AppleScript実行エラー: {}", e);
                diagnostics.applescript_stderr = Some(e.to_string());
            }
        }

        if !ax_trusted {
            return Err(PasteError::PermissionDenied(
                "AppleScript経路が失敗し、アクセシビリティ権限が未許可のためEnigoへフォールバックできませんでした。"
                    .to_string(),
            ));
        }

        info!("ペースト経路: Enigo フォールバック実行開始");
        diagnostics.used_fallback = true;
        match std::panic::catch_unwind(|| Enigo::new(&Settings::default())) {
            Ok(Ok(mut enigo)) => {
                // macOS: Cmd+V
                if let Err(e) = enigo.key(Key::Meta, Direction::Press) {
                    diagnostics.enigo_error = Some(format!("Cmd press failed: {}", e));
                    return Err(PasteError::PasteInjection(format!(
                        "Cmdキー押下に失敗: {}",
                        e
                    )));
                }
                thread::sleep(Duration::from_millis(50));
                if let Err(e) = enigo.key(Key::Other(9), Direction::Click) {
                    let _ = enigo.key(Key::Meta, Direction::Release);
                    diagnostics.enigo_error = Some(format!("V click failed: {}", e));
                    return Err(PasteError::PasteInjection(format!(
                        "Vキークリックに失敗: {}",
                        e
                    )));
                }
                thread::sleep(Duration::from_millis(50));
                if let Err(e) = enigo.key(Key::Meta, Direction::Release) {
                    diagnostics.enigo_error = Some(format!("Cmd release failed: {}", e));
                    return Err(PasteError::PasteInjection(format!(
                        "Cmdキー解放に失敗: {}",
                        e
                    )));
                }
                info!("ペースト注入完了: method=enigo");
                Ok(PasteExecutionResult {
                    method: "enigo".to_string(),
                    diagnostics,
                })
            }
            Ok(Err(e)) => Err(PasteError::PasteInjection(format!(
                "Enigoの初期化に失敗: {}",
                e
            ))),
            Err(_) => Err(PasteError::PasteInjection(
                "Enigoの初期化中にパニックが発生しました".to_string(),
            )),
        }
    }

    #[cfg(target_os = "windows")]
    {
        match Enigo::new(&Settings::default()) {
            Ok(mut enigo) => {
                // Windows: Ctrl+V
                if let Err(e) = enigo.key(Key::Control, Direction::Press) {
                    warn!("Ctrlキー押下に失敗: {}", e);
                    return Err(PasteError::PasteInjection(format!(
                        "Ctrlキー押下に失敗: {}",
                        e
                    )));
                }

                thread::sleep(Duration::from_millis(50));

                if let Err(e) = enigo.key(Key::Unicode('v'), Direction::Click) {
                    warn!("Vキークリックに失敗: {}", e);
                    let _ = enigo.key(Key::Control, Direction::Release);
                    return Err(PasteError::PasteInjection(format!(
                        "Vキークリックに失敗: {}",
                        e
                    )));
                }

                thread::sleep(Duration::from_millis(50));

                if let Err(e) = enigo.key(Key::Control, Direction::Release) {
                    warn!("Ctrlキー解放に失敗: {}", e);
                    return Err(PasteError::PasteInjection(format!(
                        "Ctrlキー解放に失敗: {}",
                        e
                    )));
                }
                return Ok(PasteExecutionResult {
                    method: "enigo".to_string(),
                    diagnostics: PasteMethodDiagnostics::default(),
                });
            }
            Err(e) => {
                warn!("Enigoの初期化に失敗: {}", e);
                return Err(PasteError::PasteInjection(format!(
                    "Enigoの初期化に失敗: {}",
                    e
                )));
            }
        }
    }

    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        Err(PasteError::PasteInjection(
            "このOSではペースト注入が未対応です".to_string(),
        ))
    }
}

#[cfg(target_os = "macos")]
pub fn type_text_to_active_app(text: &str) -> Result<TypeExecutionResult, PasteError> {
    if text.is_empty() {
        return Err(PasteError::PasteInjection(
            "空文字のため入力対象がありません".to_string(),
        ));
    }

    #[cfg(target_os = "macos")]
    {
        let mut diagnostics = TypeMethodDiagnostics::default();
        match std::panic::catch_unwind(|| Enigo::new(&Settings::default())) {
            Ok(Ok(mut enigo)) => {
                // Split long text to avoid large single injection stalls.
                for chunk in text.as_bytes().chunks(400) {
                    let s = String::from_utf8_lossy(chunk).to_string();
                    if let Err(e) = enigo.text(&s) {
                        diagnostics.enigo_error = Some(e.to_string());
                        return Err(PasteError::PasteInjection(format!("直接入力に失敗: {}", e)));
                    }
                    thread::sleep(Duration::from_millis(5));
                }
                return Ok(TypeExecutionResult {
                    method: "type_enigo".to_string(),
                    diagnostics,
                });
            }
            Ok(Err(e)) => {
                return Err(PasteError::PasteInjection(format!(
                    "Enigoの初期化に失敗: {}",
                    e
                )));
            }
            Err(_) => {
                return Err(PasteError::PasteInjection(
                    "Enigoの初期化中にパニックが発生しました".to_string(),
                ));
            }
        }
    }

    #[cfg(not(target_os = "macos"))]
    {
        Err(PasteError::PasteInjection(
            "typeモードはこのOSで未対応です".to_string(),
        ))
    }
}

/// 選択中テキストのコピーショートカットを実行（Cmd/Ctrl + C）
pub fn trigger_selection_copy_shortcut() -> Result<(), CopyError> {
    #[cfg(target_os = "macos")]
    {
        match std::process::Command::new("osascript")
            .arg("-e")
            .arg("tell application \"System Events\" to keystroke \"c\" using {command down}")
            .output()
        {
            Ok(output) if output.status.success() => return Ok(()),
            Ok(output) => {
                let stderr = String::from_utf8_lossy(&output.stderr);
                warn!("AppleScript失敗: {}", stderr);
            }
            Err(e) => {
                warn!("AppleScript実行エラー: {}", e);
            }
        }

        match std::panic::catch_unwind(|| Enigo::new(&Settings::default())) {
            Ok(Ok(mut enigo)) => {
                if let Err(e) = enigo.key(Key::Meta, Direction::Press) {
                    return Err(CopyError::Shortcut(format!("Cmdキー押下に失敗: {}", e)));
                }
                thread::sleep(Duration::from_millis(40));
                if let Err(e) = enigo.key(Key::Unicode('c'), Direction::Click) {
                    let _ = enigo.key(Key::Meta, Direction::Release);
                    return Err(CopyError::Shortcut(format!("Cキークリックに失敗: {}", e)));
                }
                thread::sleep(Duration::from_millis(40));
                if let Err(e) = enigo.key(Key::Meta, Direction::Release) {
                    return Err(CopyError::Shortcut(format!("Cmdキー解放に失敗: {}", e)));
                }
                return Ok(());
            }
            Ok(Err(e)) => return Err(CopyError::Shortcut(format!("Enigoの初期化に失敗: {}", e))),
            Err(_) => {
                return Err(CopyError::Shortcut(
                    "Enigo initialization panicked".to_string(),
                ))
            }
        }
    }

    #[cfg(target_os = "windows")]
    {
        match Enigo::new(&Settings::default()) {
            Ok(mut enigo) => {
                if let Err(e) = enigo.key(Key::Control, Direction::Press) {
                    return Err(CopyError::Shortcut(format!("Ctrlキー押下に失敗: {}", e)));
                }
                thread::sleep(Duration::from_millis(40));
                if let Err(e) = enigo.key(Key::Unicode('c'), Direction::Click) {
                    let _ = enigo.key(Key::Control, Direction::Release);
                    return Err(CopyError::Shortcut(format!("Cキークリックに失敗: {}", e)));
                }
                thread::sleep(Duration::from_millis(40));
                if let Err(e) = enigo.key(Key::Control, Direction::Release) {
                    return Err(CopyError::Shortcut(format!("Ctrlキー解放に失敗: {}", e)));
                }
                Ok(())
            }
            Err(e) => Err(CopyError::Shortcut(format!("Enigoの初期化に失敗: {}", e))),
        }
    }

    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        Err(CopyError::Shortcut(
            "このOSでは選択コピーショートカットが未対応です".to_string(),
        ))
    }
}
