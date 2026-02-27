#[cfg(not(target_os = "macos"))]
use cpal::traits::{DeviceTrait, HostTrait};
use std::path::PathBuf;
use std::thread;
use std::time::Duration;
use tauri::{AppHandle, Emitter};
use thiserror::Error;
#[cfg(target_os = "macos")]
use tracing::warn;
use tracing::{debug, error};

#[cfg(target_os = "macos")]
use crate::AppState;
#[cfg(target_os = "windows")]
use cpal::traits::StreamTrait;
#[cfg(target_os = "macos")]
use serde_json::json;
#[cfg(target_os = "macos")]
use std::io::{BufRead, BufReader};
#[cfg(target_os = "macos")]
use std::io::{Read, Write};
#[cfg(not(target_os = "windows"))]
use std::path::Path;
#[cfg(all(not(target_os = "macos"), not(target_os = "windows")))]
use std::process::Child;
#[cfg(target_os = "windows")]
use std::sync::{Arc, Mutex};
#[cfg(target_os = "macos")]
use tauri::Manager;

#[cfg(target_os = "macos")]
use std::ffi::CString;
#[cfg(target_os = "macos")]
use std::fs::File;
#[cfg(target_os = "macos")]
use std::os::unix::ffi::OsStrExt;
#[cfg(target_os = "macos")]
use std::os::unix::io::FromRawFd;
#[cfg(target_os = "macos")]
use std::os::unix::process::ExitStatusExt;

#[cfg(target_os = "macos")]
const SIDECAR_EVENT_TRANSCRIPTION: &str = "transcription";
#[cfg(target_os = "macos")]
const SIDECAR_EVENT_AUDIO_LEVEL: &str = "audio-level";
#[cfg(target_os = "macos")]
const SIDECAR_EVENT_ERROR: &str = "error";
#[cfg(target_os = "macos")]
const SIDECAR_EVENT_DEBUG: &str = "debug";
#[cfg(target_os = "macos")]
const APP_EVENT_PARTIAL_TRANSCRIPTION: &str = "partial-transcription";
#[cfg(target_os = "macos")]
const APP_EVENT_RECORDING_ERROR: &str = "recording-error";

#[cfg(target_os = "macos")]
fn resolve_speech_locale() -> &'static str {
    match crate::config::settings::get_language().as_str() {
        "en" => "en-US",
        "ja" => "ja-JP",
        _ => "ja-JP",
    }
}

#[cfg(target_os = "macos")]
fn find_speech_sidecar_path(app: &AppHandle) -> Result<PathBuf, RecorderError> {
    let candidates = crate::overlay::macos_sidecar_candidates(app, "speech-recognizer");
    candidates
        .iter()
        .find(|p| p.exists())
        .cloned()
        .ok_or_else(|| {
            error!("Sidecar binary not found in candidates: {:?}", candidates);
            RecorderError::ProcessStart("speech-recognizer sidecar not found".to_string())
        })
}

#[cfg(target_os = "macos")]
fn create_temp_recording_output_path() -> PathBuf {
    let temp_dir = std::env::temp_dir();
    let timestamp = crate::util::current_unix_timestamp_secs();
    temp_dir.join(format!("koetype_recording_{}.wav", timestamp))
}

#[cfg(target_os = "macos")]
fn start_speech_sidecar(
    app: &AppHandle,
    output_path: &Path,
) -> Result<RecorderChild, RecorderError> {
    let output_path_str = output_path.to_string_lossy().to_string();
    let sidecar_path = find_speech_sidecar_path(app)?;
    debug!(
        "Starting sidecar: {:?} output={}",
        sidecar_path, output_path_str
    );

    let locale = resolve_speech_locale();
    let enable_speech = true;
    spawn_sidecar(&sidecar_path, &output_path_str, locale, enable_speech).map_err(|e| {
        RecorderError::ProcessStart(format!("Failed to spawn {:?}: {}", sidecar_path, e))
    })
}

#[cfg(target_os = "macos")]
fn attach_sidecar_event_pumps(child: &mut RecorderChild, app: &AppHandle) {
    if let Some(stderr) = child.take_stderr() {
        spawn_sidecar_stderr_pump(stderr);
    }

    if let Some(stdout) = child.take_stdout() {
        spawn_sidecar_stdout_pump(stdout, app.clone());
    }
}

#[cfg(target_os = "macos")]
fn request_sidecar_stop(child: &mut RecorderChild) {
    if let Some(mut stdin) = child.take_stdin() {
        let _ = writeln!(stdin);
    }
}

#[cfg(target_os = "macos")]
fn wait_for_recording_output(output_path: &Path) -> Result<(), RecorderError> {
    for _ in 0..10 {
        if let Ok(metadata) = std::fs::metadata(output_path) {
            if metadata.len() > 0 {
                return Ok(());
            }
        }
        thread::sleep(Duration::from_millis(50));
    }

    Err(RecorderError::Io(
        "音声ファイルが作成されていません。マイク/音声認識の許可やSwiftサイドカーの実行状態を確認してください。"
            .to_string(),
    ))
}

/// Audio Device Info (Mock for now as Swift script uses default)
#[derive(serde::Serialize, Clone, Debug)]
pub struct AudioDevice {
    pub name: String,
    pub id: String,
    pub is_default: bool,
}

/// List available input devices
#[cfg(target_os = "macos")]
pub fn list_input_devices() -> Vec<AudioDevice> {
    // macOS 版の録音は speech-recognizer sidecar が担当するため、
    // メインプロセスではマイクAPIに触れず権限要求主体を sidecar に一本化する。
    vec![AudioDevice {
        name: "System Default".to_string(),
        id: "default".to_string(),
        is_default: true,
    }]
}

/// List available input devices
#[cfg(not(target_os = "macos"))]
pub fn list_input_devices() -> Vec<AudioDevice> {
    let host = cpal::default_host();
    let default_device_name = host.default_input_device().and_then(|d| d.name().ok());

    let mut devices = Vec::new();

    if let Ok(input_devices) = host.input_devices() {
        for device in input_devices {
            if let Ok(name) = device.name() {
                let is_default = default_device_name
                    .as_ref()
                    .map(|d| d == &name)
                    .unwrap_or(false);
                devices.push(AudioDevice {
                    name: name.clone(),
                    id: name,
                    is_default,
                });
            }
        }
    }

    // デバイスが取得できなかった場合のフォールバック
    if devices.is_empty() {
        devices.push(AudioDevice {
            name: "System Default".to_string(),
            id: "default".to_string(),
            is_default: true,
        });
    }

    devices
}

#[derive(Error, Debug)]
pub enum RecorderError {
    #[error("Failed to start recording process: {0}")]
    ProcessStart(String),

    #[error("Not recording")]
    NotRecording,

    #[error("Already recording")]
    AlreadyRecording,

    #[error("IO Error: {0}")]
    Io(String),
}

pub struct AudioRecorder {
    child: Option<RecorderChild>,
    output_path: Option<PathBuf>,
    selected_device: Option<String>,
}

// AudioRecorder is Send because Child is Send.
unsafe impl Send for AudioRecorder {}

#[cfg(target_os = "macos")]
fn update_pending_os_text(app: &AppHandle, text: &str) {
    if let Ok(mut pending) = app.state::<AppState>().pending_os_text.lock() {
        let should_update = match pending.as_ref() {
            Some(existing) => existing != text,
            None => true,
        };
        if should_update {
            *pending = Some(text.to_string());
        }
    }
}

#[cfg(target_os = "macos")]
fn forward_partial_transcription(app: &AppHandle, text: &str) {
    update_pending_os_text(app, text);
    let _ = app.emit(APP_EVENT_PARTIAL_TRANSCRIPTION, text);
    crate::overlay::overlay_send_json(
        app,
        json!({
            "type": APP_EVENT_PARTIAL_TRANSCRIPTION,
            "text": text
        }),
    );
}

#[cfg(target_os = "macos")]
fn forward_audio_level(app: &AppHandle, level: f64) {
    let _ = app.emit(SIDECAR_EVENT_AUDIO_LEVEL, level);
    crate::overlay::overlay_send_json(
        app,
        json!({
            "type": SIDECAR_EVENT_AUDIO_LEVEL,
            "level": level
        }),
    );
}

#[cfg(target_os = "macos")]
fn forward_recording_error(app: &AppHandle, msg: &str) {
    error!("Sidecar error: {}", msg);
    let _ = app.emit(APP_EVENT_RECORDING_ERROR, msg);
    crate::overlay::overlay_send_json(
        app,
        json!({
            "type": APP_EVENT_RECORDING_ERROR,
            "message": msg
        }),
    );
}

#[cfg(target_os = "macos")]
fn handle_sidecar_event(app: &AppHandle, event: &serde_json::Value) {
    let Some(type_str) = event["type"].as_str() else {
        return;
    };

    match type_str {
        SIDECAR_EVENT_TRANSCRIPTION => {
            if let Some(text) = event["text"].as_str() {
                forward_partial_transcription(app, text);
            }
        }
        SIDECAR_EVENT_AUDIO_LEVEL => {
            if let Some(level) = event["level"].as_f64() {
                forward_audio_level(app, level);
            }
        }
        SIDECAR_EVENT_ERROR => {
            if let Some(msg) = event["message"].as_str() {
                forward_recording_error(app, msg);
            }
        }
        SIDECAR_EVENT_DEBUG => {
            if let Some(msg) = event["message"].as_str() {
                debug!("Sidecar debug: {}", msg);
            }
        }
        _ => {}
    }
}

#[cfg(target_os = "macos")]
fn spawn_sidecar_stderr_pump(stderr: Box<dyn Read + Send>) {
    thread::spawn(move || {
        let reader = BufReader::new(stderr);
        for line in reader.lines() {
            if let Ok(line) = line {
                warn!("Sidecar stderr: {}", line);
            }
        }
    });
}

#[cfg(target_os = "macos")]
fn spawn_sidecar_stdout_pump(stdout: Box<dyn Read + Send>, app_handle: AppHandle) {
    thread::spawn(move || {
        let reader = BufReader::new(stdout);
        for line in reader.lines() {
            match line {
                Ok(line) => {
                    if let Ok(event) = serde_json::from_str::<serde_json::Value>(&line) {
                        handle_sidecar_event(&app_handle, &event);
                    }
                }
                Err(_) => break,
            }
        }
    });
}

impl AudioRecorder {
    pub fn new() -> Self {
        Self {
            child: None,
            output_path: None,
            selected_device: None,
        }
    }

    /// Set device (Store preference, though currently script uses default)
    pub fn set_device(&mut self, device_name: Option<String>) {
        self.selected_device = device_name;
    }

    /// Start recording (macOS: Swift sidecar)
    #[cfg(target_os = "macos")]
    pub fn start_recording(&mut self, app: AppHandle) -> Result<(), RecorderError> {
        if self.child.is_some() {
            return Err(RecorderError::AlreadyRecording);
        }

        let output_path = create_temp_recording_output_path();
        self.output_path = Some(output_path.clone());

        let mut child = start_speech_sidecar(&app, &output_path)?;
        attach_sidecar_event_pumps(&mut child, &app);

        self.child = Some(child);
        debug!("Recording started (Swift Sidecar)");

        Ok(())
    }

    /// Start recording (Windows: cpal direct recording)
    #[cfg(target_os = "windows")]
    pub fn start_recording(&mut self, app: AppHandle) -> Result<(), RecorderError> {
        if self.child.is_some() {
            return Err(RecorderError::AlreadyRecording);
        }

        // Temp file path
        let temp_dir = std::env::temp_dir();
        let timestamp = crate::util::current_unix_timestamp_secs();
        let output_path = temp_dir.join(format!("koetype_recording_{}.wav", timestamp));
        self.output_path = Some(output_path.clone());

        debug!("Starting cpal recording: {:?}", output_path);

        // Setup cpal recording
        let host = cpal::default_host();
        let device = host.default_input_device().ok_or_else(|| {
            RecorderError::ProcessStart("デフォルト入力デバイスが見つかりません".to_string())
        })?;

        let config = device
            .default_input_config()
            .map_err(|e| RecorderError::ProcessStart(format!("入力設定の取得に失敗: {}", e)))?;

        debug!("Recording config: {:?}", config);

        // Create WAV writer
        let spec = hound::WavSpec {
            channels: config.channels(),
            sample_rate: config.sample_rate().0,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };

        let writer = hound::WavWriter::create(&output_path, spec)
            .map_err(|e| RecorderError::ProcessStart(format!("WAVファイルの作成に失敗: {}", e)))?;

        let writer_handle = Arc::new(Mutex::new(Some(writer)));
        let writer_clone = writer_handle.clone();

        // Build the input stream
        let stream = match config.sample_format() {
            cpal::SampleFormat::F32 => {
                build_input_stream::<f32>(&device, &config.into(), writer_clone, app)?
            }
            cpal::SampleFormat::I16 => {
                build_input_stream::<i16>(&device, &config.into(), writer_clone, app)?
            }
            cpal::SampleFormat::U16 => {
                build_input_stream::<u16>(&device, &config.into(), writer_clone, app)?
            }
            _ => {
                return Err(RecorderError::ProcessStart(
                    "サポートされていないサンプルフォーマット".to_string(),
                ))
            }
        };

        stream
            .play()
            .map_err(|e| RecorderError::ProcessStart(format!("録音の開始に失敗: {}", e)))?;

        self.child = Some(RecorderChild::CpalStream(CpalRecorder {
            _stream: stream,
            writer_handle,
        }));

        debug!("Recording started (cpal)");

        Ok(())
    }

    /// Stop recording and return path (macOS: sidecar)
    #[cfg(target_os = "macos")]
    pub fn stop_recording(&mut self) -> Result<PathBuf, RecorderError> {
        if let Some(mut child) = self.child.take() {
            request_sidecar_stop(&mut child);

            let status = child.wait().map_err(|e| RecorderError::Io(e))?;
            if !status.success() {
                return Err(RecorderError::Io(format!(
                    "録音プロセスが異常終了しました (status: {})",
                    status
                )));
            }

            let output_path = self.output_path.take().ok_or(RecorderError::NotRecording)?;

            wait_for_recording_output(&output_path)?;

            debug!("Recording stopped: {:?}", output_path);

            Ok(output_path)
        } else {
            Err(RecorderError::NotRecording)
        }
    }

    /// Stop recording and return path (Windows: cpal)
    #[cfg(target_os = "windows")]
    pub fn stop_recording(&mut self) -> Result<PathBuf, RecorderError> {
        if let Some(RecorderChild::CpalStream(recorder)) = self.child.take() {
            // Finalize WAV file
            if let Ok(mut writer_guard) = recorder.writer_handle.lock() {
                if let Some(writer) = writer_guard.take() {
                    writer.finalize().map_err(|e| {
                        RecorderError::Io(format!("WAVファイルの終了処理に失敗: {}", e))
                    })?;
                }
            }

            // Stream is automatically stopped when dropped
            let output_path = self.output_path.take().ok_or(RecorderError::NotRecording)?;

            // Verify file exists and has data
            thread::sleep(Duration::from_millis(100));

            let metadata = std::fs::metadata(&output_path)
                .map_err(|e| RecorderError::Io(format!("音声ファイルの確認に失敗: {}", e)))?;

            if metadata.len() == 0 {
                return Err(RecorderError::Io(
                    "音声ファイルが空です。マイクの許可を確認してください。".to_string(),
                ));
            }

            debug!(
                "Recording stopped: {:?} ({} bytes)",
                output_path,
                metadata.len()
            );

            Ok(output_path)
        } else {
            Err(RecorderError::NotRecording)
        }
    }
}

impl Default for AudioRecorder {
    fn default() -> Self {
        Self::new()
    }
}

enum RecorderChild {
    #[cfg(all(not(target_os = "macos"), not(target_os = "windows")))]
    Std(Child),
    #[cfg(target_os = "macos")]
    Posix(PosixChild),
    #[cfg(target_os = "windows")]
    CpalStream(CpalRecorder),
}

#[cfg(target_os = "windows")]
struct CpalRecorder {
    _stream: cpal::Stream,
    writer_handle: Arc<Mutex<Option<hound::WavWriter<std::io::BufWriter<std::fs::File>>>>>,
}

#[cfg(target_os = "macos")]
impl RecorderChild {
    fn take_stdin(&mut self) -> Option<Box<dyn Write + Send>> {
        match self {
            #[cfg(target_os = "macos")]
            RecorderChild::Posix(child) => child.stdin.take().map(|s| Box::new(s) as _),
        }
    }

    fn take_stdout(&mut self) -> Option<Box<dyn Read + Send>> {
        match self {
            #[cfg(target_os = "macos")]
            RecorderChild::Posix(child) => child.stdout.take().map(|s| Box::new(s) as _),
        }
    }

    fn take_stderr(&mut self) -> Option<Box<dyn Read + Send>> {
        match self {
            #[cfg(target_os = "macos")]
            RecorderChild::Posix(child) => child.stderr.take().map(|s| Box::new(s) as _),
        }
    }

    fn wait(&mut self) -> Result<std::process::ExitStatus, String> {
        match self {
            #[cfg(target_os = "macos")]
            RecorderChild::Posix(child) => child.wait(),
        }
    }
}

#[cfg(target_os = "macos")]
struct PosixChild {
    pid: libc::pid_t,
    stdin: Option<File>,
    stdout: Option<File>,
    stderr: Option<File>,
}

#[cfg(target_os = "macos")]
impl PosixChild {
    fn wait(&mut self) -> Result<std::process::ExitStatus, String> {
        let mut status: libc::c_int = 0;
        let rc = unsafe { libc::waitpid(self.pid, &mut status, 0) };
        if rc < 0 {
            return Err(std::io::Error::last_os_error().to_string());
        }
        Ok(std::process::ExitStatus::from_raw(status))
    }
}

#[cfg(target_os = "macos")]
pub(crate) unsafe fn set_responsibility_disclaim(attr: &mut libc::posix_spawnattr_t) {
    type DisclaimFn =
        unsafe extern "C" fn(*mut libc::posix_spawnattr_t, libc::c_int) -> libc::c_int;
    let Ok(symbol) = CString::new("responsibility_spawnattrs_setdisclaim") else {
        return;
    };
    let func = libc::dlsym(libc::RTLD_DEFAULT, symbol.as_ptr());
    if !func.is_null() {
        let f: DisclaimFn = std::mem::transmute(func);
        let _ = f(attr as *mut _, 1);
    }
}

#[cfg(target_os = "macos")]
fn open_spawn_pipes() -> Result<([i32; 2], [i32; 2], [i32; 2]), String> {
    let mut stdin_fds = [0; 2];
    let mut stdout_fds = [0; 2];
    let mut stderr_fds = [0; 2];

    unsafe {
        if libc::pipe(stdin_fds.as_mut_ptr()) != 0
            || libc::pipe(stdout_fds.as_mut_ptr()) != 0
            || libc::pipe(stderr_fds.as_mut_ptr()) != 0
        {
            return Err(std::io::Error::last_os_error().to_string());
        }
    }

    Ok((stdin_fds, stdout_fds, stderr_fds))
}

#[cfg(target_os = "macos")]
fn close_spawn_pipes(stdin_fds: [i32; 2], stdout_fds: [i32; 2], stderr_fds: [i32; 2]) {
    unsafe {
        libc::close(stdin_fds[0]);
        libc::close(stdin_fds[1]);
        libc::close(stdout_fds[0]);
        libc::close(stdout_fds[1]);
        libc::close(stderr_fds[0]);
        libc::close(stderr_fds[1]);
    }
}

#[cfg(target_os = "macos")]
fn close_parent_pipe_ends(stdin_fds: [i32; 2], stdout_fds: [i32; 2], stderr_fds: [i32; 2]) {
    unsafe {
        libc::close(stdin_fds[0]);
        libc::close(stdout_fds[1]);
        libc::close(stderr_fds[1]);
    }
}

#[cfg(target_os = "macos")]
fn spawn_sidecar(
    path: &Path,
    output_path: &str,
    locale: &str,
    enable_speech: bool,
) -> Result<RecorderChild, String> {
    let (stdin_fds, stdout_fds, stderr_fds) = open_spawn_pipes()?;

    let mut actions: libc::posix_spawn_file_actions_t = unsafe { std::mem::zeroed() };
    unsafe {
        libc::posix_spawn_file_actions_init(&mut actions);
        libc::posix_spawn_file_actions_adddup2(&mut actions, stdin_fds[0], libc::STDIN_FILENO);
        libc::posix_spawn_file_actions_adddup2(&mut actions, stdout_fds[1], libc::STDOUT_FILENO);
        libc::posix_spawn_file_actions_adddup2(&mut actions, stderr_fds[1], libc::STDERR_FILENO);

        libc::posix_spawn_file_actions_addclose(&mut actions, stdin_fds[1]);
        libc::posix_spawn_file_actions_addclose(&mut actions, stdout_fds[0]);
        libc::posix_spawn_file_actions_addclose(&mut actions, stderr_fds[0]);
    }

    let mut attr: libc::posix_spawnattr_t = unsafe { std::mem::zeroed() };
    unsafe {
        libc::posix_spawnattr_init(&mut attr);
        set_responsibility_disclaim(&mut attr);
    }

    let c_path = CString::new(path.as_os_str().as_bytes()).map_err(|e| e.to_string())?;
    let c_output = CString::new(output_path).map_err(|e| e.to_string())?;
    let c_locale = CString::new(locale).map_err(|e| e.to_string())?;
    let mut argv: Vec<*mut libc::c_char> = vec![
        c_path.as_ptr() as *mut libc::c_char,
        c_output.as_ptr() as *mut libc::c_char,
        c_locale.as_ptr() as *mut libc::c_char,
    ];
    let speech_flag = if enable_speech {
        Some(CString::new("--speech").map_err(|e| e.to_string())?)
    } else {
        None
    };
    if let Some(flag) = speech_flag.as_ref() {
        argv.push(flag.as_ptr() as *mut libc::c_char);
    }
    argv.push(std::ptr::null_mut());

    let mut pid: libc::pid_t = 0;
    let envp = std::ptr::null();
    let rc = unsafe {
        libc::posix_spawn(
            &mut pid,
            c_path.as_ptr(),
            &actions,
            &attr,
            argv.as_mut_ptr(),
            envp,
        )
    };

    unsafe {
        libc::posix_spawn_file_actions_destroy(&mut actions);
        libc::posix_spawnattr_destroy(&mut attr);
    }

    if rc != 0 {
        close_spawn_pipes(stdin_fds, stdout_fds, stderr_fds);
        return Err(std::io::Error::from_raw_os_error(rc).to_string());
    }

    close_parent_pipe_ends(stdin_fds, stdout_fds, stderr_fds);

    let child = PosixChild {
        pid,
        stdin: Some(unsafe { File::from_raw_fd(stdin_fds[1]) }),
        stdout: Some(unsafe { File::from_raw_fd(stdout_fds[0]) }),
        stderr: Some(unsafe { File::from_raw_fd(stderr_fds[0]) }),
    };

    Ok(RecorderChild::Posix(child))
}

#[cfg(all(not(target_os = "macos"), not(target_os = "windows")))]
fn spawn_sidecar(
    path: &Path,
    output_path: &str,
    locale: &str,
    enable_speech: bool,
) -> Result<RecorderChild, String> {
    use std::process::{Command, Stdio};
    let mut command = Command::new(path);
    command
        .arg(output_path)
        .arg(locale)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .stdin(Stdio::piped());
    if enable_speech {
        command.arg("--speech");
    }

    let child = command.spawn().map_err(|e| e.to_string())?;
    Ok(RecorderChild::Std(child))
}

#[cfg(target_os = "windows")]
fn write_samples_to_wav<T>(
    data: &[T],
    writer: &Arc<Mutex<Option<hound::WavWriter<std::io::BufWriter<std::fs::File>>>>>,
) where
    T: cpal::Sample,
    f32: From<T>,
{
    if let Ok(mut writer_guard) = writer.lock() {
        if let Some(writer) = writer_guard.as_mut() {
            for &sample in data {
                let sample_f32: f32 = sample.into();
                let sample_i16 = (sample_f32 * i16::MAX as f32) as i16;
                let _ = writer.write_sample(sample_i16);
            }
        }
    }
}

#[cfg(target_os = "windows")]
fn emit_audio_level_event<T>(data: &[T], app: &AppHandle)
where
    T: cpal::Sample,
    f32: From<T>,
{
    let rms = calculate_rms(data);
    let _ = app.emit("audio-level", rms);
}

#[cfg(target_os = "windows")]
fn build_input_stream<T>(
    device: &cpal::Device,
    config: &cpal::StreamConfig,
    writer: Arc<Mutex<Option<hound::WavWriter<std::io::BufWriter<std::fs::File>>>>>,
    app: AppHandle,
) -> Result<cpal::Stream, RecorderError>
where
    T: cpal::Sample + cpal::SizedSample,
    f32: From<T>,
{
    let err_fn = |err| error!("Recording stream error: {}", err);

    let stream = device
        .build_input_stream(
            config,
            move |data: &[T], _: &cpal::InputCallbackInfo| {
                write_samples_to_wav(data, &writer);
                emit_audio_level_event(data, &app);
            },
            err_fn,
            None,
        )
        .map_err(|e| RecorderError::ProcessStart(format!("入力ストリームの構築に失敗: {}", e)))?;

    Ok(stream)
}

#[cfg(target_os = "windows")]
fn calculate_rms<T>(data: &[T]) -> f64
where
    T: cpal::Sample,
    f32: From<T>,
{
    let sum: f32 = data
        .iter()
        .map(|&s| {
            let f: f32 = s.into();
            f * f
        })
        .sum();
    (sum / data.len() as f32).sqrt() as f64
}
