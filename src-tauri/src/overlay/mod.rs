use tauri::{AppHandle, Manager};

#[cfg(target_os = "macos")]
use std::path::PathBuf;
#[cfg(target_os = "macos")]
use tracing::{debug, info, warn};

#[cfg(target_os = "macos")]
use std::io::Write;

/// 現在のフローティングウィンドウサイズとモニター情報から、論理座標での中央下配置 x 座標を返す
#[cfg(target_os = "macos")]
fn compute_center_x(window: &tauri::WebviewWindow) -> Option<f64> {
    let monitor = window
        .primary_monitor()
        .ok()
        .flatten()
        .or_else(|| window.current_monitor().ok().flatten())?;
    let scale_factor: f64 = monitor.scale_factor();
    let work_area = monitor.work_area();
    let work_size = work_area.size.to_logical::<f64>(scale_factor);
    let work_pos = work_area.position.to_logical::<f64>(scale_factor);
    let window_size = window.outer_size().ok()?.to_logical::<f64>(scale_factor);
    Some(work_pos.x + (work_size.width - window_size.width) / 2.0)
}

#[cfg(target_os = "macos")]
const KOETYPE_SPEECH_RECOGNIZER_APP_BUNDLE: &str = "KoeType Speech Recognizer.app";

#[cfg(target_os = "macos")]
pub(crate) struct OverlayProcess {
    pub(crate) child: std::process::Child,
    pub(crate) stdin: std::process::ChildStdin,
}

#[cfg(target_os = "macos")]
fn cleanup_overlay_process(slot: &mut Option<OverlayProcess>) {
    if let Some(mut process) = slot.take() {
        let _ = process.child.kill();
    }
}

#[cfg(not(target_os = "macos"))]
pub(crate) struct OverlayProcess;

#[cfg(target_os = "macos")]
pub(crate) fn overlay_send_json(app: &AppHandle, value: serde_json::Value) {
    const WRITE_FAILED_LABEL: &str = "write failed";
    const RETRY_FAILED_LABEL: &str = "retry failed";

    fn write_event(process: &mut OverlayProcess, line: &str) -> Result<(), std::io::Error> {
        process.stdin.write_all(line.as_bytes())?;
        process.stdin.write_all(b"\n")?;
        process.stdin.flush()
    }

    fn try_send_with_guard(
        app: &AppHandle,
        line: &str,
        event_type: &str,
        failure_log_label: &str,
    ) -> Result<(), ()> {
        let state = app.state::<crate::AppState>();
        if let Ok(mut guard) = state.overlay.lock() {
            if let Some(process) = guard.as_mut() {
                if let Err(e) = write_event(process, line) {
                    warn!(
                        "overlay_send_json {} (type={}): {}",
                        failure_log_label, event_type, e
                    );
                    cleanup_overlay_process(&mut guard);
                    return Err(());
                }
                return Ok(());
            }
        };
        Err(())
    }

    let event_type = value
        .get("type")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown");
    let line = match serde_json::to_string(&value) {
        Ok(line) => line,
        Err(e) => {
            warn!(
                "overlay_send_json serialize failed (type={}): {}",
                event_type, e
            );
            return;
        }
    };

    if try_send_with_guard(app, &line, event_type, WRITE_FAILED_LABEL).is_ok() {
        return;
    }

    start_overlay_sidecar(app);

    if try_send_with_guard(app, &line, event_type, RETRY_FAILED_LABEL).is_ok() {
        info!(
            "overlay_send_json recovered after sidecar restart (type={})",
            event_type
        );
    } else {
        warn!(
            "overlay_send_json restart attempted but overlay is still unavailable (type={})",
            event_type
        );
    }
}

#[cfg(not(target_os = "macos"))]
pub(crate) fn overlay_send_json(_app: &AppHandle, _value: serde_json::Value) {}

#[cfg(target_os = "macos")]
pub(crate) fn macos_sidecar_candidates(app: &AppHandle, stem: &str) -> Vec<PathBuf> {
    fn push_sidecar_layout(base: &std::path::Path, stem: &str, out: &mut Vec<PathBuf>) {
        if stem == "speech-recognizer" {
            out.push(
                base.join(KOETYPE_SPEECH_RECOGNIZER_APP_BUNDLE)
                    .join("Contents/MacOS")
                    .join(stem),
            );
            out.push(
                base.join("bin")
                    .join(KOETYPE_SPEECH_RECOGNIZER_APP_BUNDLE)
                    .join("Contents/MacOS")
                    .join(stem),
            );
        } else {
            out.push(base.join(format!("{stem}.app/Contents/MacOS/{stem}")));
            out.push(
                base.join("bin")
                    .join(format!("{stem}.app/Contents/MacOS/{stem}")),
            );
        }
        out.push(base.join(stem));
        out.push(base.join("bin").join(stem));
    }

    let mut candidates = Vec::new();

    if let Ok(resource_dir) = app.path().resource_dir() {
        push_sidecar_layout(&resource_dir, stem, &mut candidates);
    }

    if let Ok(exe_path) = std::env::current_exe() {
        if let Some(macos_dir) = exe_path.parent() {
            push_sidecar_layout(macos_dir, stem, &mut candidates);
            if let Some(contents_dir) = macos_dir.parent() {
                push_sidecar_layout(&contents_dir.join("Resources"), stem, &mut candidates);
            }
        }
    }

    if let Ok(current_dir) = std::env::current_dir() {
        push_sidecar_layout(&current_dir, stem, &mut candidates);
        push_sidecar_layout(&current_dir.join("src-tauri"), stem, &mut candidates);
        push_sidecar_layout(&current_dir.join("../src-tauri"), stem, &mut candidates);
    }

    candidates
}

#[cfg(target_os = "macos")]
fn find_existing_sidecar_path(app: &AppHandle, stem: &str) -> (Vec<PathBuf>, Option<PathBuf>) {
    let candidates = macos_sidecar_candidates(app, stem);
    let path = candidates.iter().find(|p| p.exists()).cloned();
    (candidates, path)
}

#[cfg(target_os = "macos")]
pub(crate) fn find_macos_sidecar_path(
    app: &AppHandle,
    stem: &str,
) -> (Vec<PathBuf>, Option<PathBuf>) {
    find_existing_sidecar_path(app, stem)
}

/// エラー発生時などにフローティングウィンドウを遅延後に閉じる
pub(crate) fn close_floating_window_after_delay(app: &AppHandle, delay_ms: u64) {
    let app_clone = app.clone();
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(tokio::time::Duration::from_millis(delay_ms)).await;

        if let Some(window) = app_clone.get_webview_window("floating") {
            let _ = window.hide();
        }

        #[cfg(target_os = "macos")]
        {
            overlay_send_json(
                &app_clone,
                serde_json::json!({
                    "type": "hide"
                }),
            );
        }
    });
}

#[cfg(target_os = "macos")]
pub(crate) fn start_overlay_sidecar(app: &AppHandle) {
    let state = app.state::<crate::AppState>();
    if let Ok(mut guard) = state.overlay.lock() {
        if let Some(process) = guard.as_mut() {
            match process.child.try_wait() {
                Ok(Some(status)) => {
                    info!(
                        "floating-overlay process was stale; restarting. exit_status={}",
                        status
                    );
                    *guard = None;
                }
                Ok(None) => {
                    return;
                }
                Err(e) => {
                    warn!(
                        "floating-overlay try_wait failed; restarting process: {}",
                        e
                    );
                    *guard = None;
                }
            }
        }
    }

    let (candidates, maybe_sidecar_path) = find_existing_sidecar_path(app, "floating-overlay");
    let Some(sidecar_path) = maybe_sidecar_path else {
        warn!(
            "floating-overlay sidecar not found. candidates={:?}",
            candidates
        );
        return;
    };
    info!(
        "starting floating-overlay sidecar: {}",
        sidecar_path.display()
    );

    #[cfg(debug_assertions)]
    let stderr_stdio = std::process::Stdio::piped();
    #[cfg(not(debug_assertions))]
    let stderr_stdio = std::process::Stdio::null();

    let child_result = std::process::Command::new(&sidecar_path)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::null())
        .stderr(stderr_stdio)
        .spawn();

    if let Ok(mut child) = child_result {
        let pid = child.id();

        #[cfg(debug_assertions)]
        if let Some(stderr) = child.stderr.take() {
            std::thread::spawn(move || {
                use std::io::BufRead;
                let reader = std::io::BufReader::new(stderr);
                for line in reader.lines().flatten() {
                    warn!("[floating-overlay stderr] {}", line);
                }
            });
        }

        if let Some(stdin) = child.stdin.take() {
            if let Ok(mut guard) = state.overlay.lock() {
                *guard = Some(OverlayProcess { child, stdin });
                debug!("floating-overlay sidecar started pid={}", pid);
            };
        } else {
            warn!(
                "floating-overlay sidecar spawned without stdin: {}",
                sidecar_path.display()
            );
            let _ = child.kill();
        }
    } else if let Err(e) = child_result {
        warn!(
            "failed to spawn floating-overlay sidecar ({}): {}",
            sidecar_path.display(),
            e
        );
    }
}

#[cfg(target_os = "macos")]
pub(crate) fn shutdown_overlay(app: &AppHandle) {
    let state = app.state::<crate::AppState>();
    if let Ok(mut guard) = state.overlay.lock() {
        if let Some(process) = guard.as_ref() {
            debug!("shutdown_overlay kill pid={}", process.child.id());
        }
        cleanup_overlay_process(&mut guard);
    };
}

#[cfg(target_os = "macos")]
fn send_overlay_position_with_retry(app: &AppHandle, window: &tauri::WebviewWindow) {
    if let Some(x) = compute_center_x(window) {
        for _ in 0..3 {
            overlay_send_json(app, serde_json::json!({ "type": "position", "x": x }));
            std::thread::sleep(std::time::Duration::from_millis(30));
        }
    }
}

#[cfg(target_os = "macos")]
pub(crate) fn apply_platform_floating_ui(app: &AppHandle) {
    start_overlay_sidecar(app);
    if let Some(window) = app.get_webview_window("floating") {
        send_overlay_position_with_retry(app, &window);
        let _ = window.hide();
    }
}

#[cfg(target_os = "macos")]
// See comment above: scoped allowance for cocoa until objc2 migration is possible.
pub(crate) fn configure_macos_floating_window(window: &tauri::WebviewWindow) {
    use objc::runtime::Object;
    use objc::{msg_send, sel, sel_impl};
    use std::env;

    let Ok(raw_window) = window.ns_window() else {
        warn!("failed to acquire ns_window for floating window configuration");
        return;
    };
    let ns_window = raw_window as *mut Object;
    if !ns_window.is_null() {
        unsafe {
            const NS_FLOATING_WINDOW_LEVEL: i64 = 3;
            const NS_STATUS_WINDOW_LEVEL: i64 = 25;
            const NS_POPUP_MENU_WINDOW_LEVEL: i64 = 101;
            const NS_SCREENSAVER_WINDOW_LEVEL: i64 = 1000;

            let level_env = env::var("KOETYPE_OVERLAY_LEVEL").ok();
            let window_level = match level_env.as_deref() {
                Some("floating") => NS_FLOATING_WINDOW_LEVEL,
                Some("status") => NS_STATUS_WINDOW_LEVEL,
                Some("popup") => NS_POPUP_MENU_WINDOW_LEVEL,
                Some("screensaver") => NS_SCREENSAVER_WINDOW_LEVEL,
                _ => NS_STATUS_WINDOW_LEVEL,
            };

            const CAN_JOIN_ALL_SPACES: u64 = 1 << 0;
            const STATIONARY: u64 = 1 << 4;
            const IGNORES_CYCLE: u64 = 1 << 6;
            const FULL_SCREEN_AUXILIARY: u64 = 1 << 8;
            let collection_behavior =
                CAN_JOIN_ALL_SPACES | FULL_SCREEN_AUXILIARY | STATIONARY | IGNORES_CYCLE;
            let _: () = msg_send![ns_window, setCollectionBehavior: collection_behavior];

            let _: () = msg_send![ns_window, setLevel: window_level];
            let _: () = msg_send![ns_window, setHidesOnDeactivate: false];
            let _: () = msg_send![ns_window, setAcceptsMouseMovedEvents: true];
        }
    }
}
