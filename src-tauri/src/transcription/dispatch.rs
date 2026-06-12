use crate::config::settings::SttProvider;
use crate::error_log::ErrorType;
use tracing::{info, warn};

/// 文字起こし実行の結果。
pub struct DispatchOutcome {
    pub result: Result<String, String>,
    /// Collaborate モードで Whisper が返した生テキスト（履歴のサブ枠表示用）
    pub whisper_text: Option<String>,
    /// ユーザーへの結果提供を優先するため、ペースト完了後にまとめて書き込むエラーログ
    /// (error_type, message, context)
    pub deferred_error_logs: Vec<(ErrorType, String, String)>,
}

async fn transcribe_with_gemini(audio_path: &str) -> Result<String, String> {
    let api_key = crate::config::get_api_key()
        .map_err(|_| "APIキーが設定されていません".to_string())?;
    let model = crate::config::get_model();
    crate::transcription::transcribe_audio_with_model(audio_path, &api_key, &model)
        .await
        .map_err(|e| e.to_string())
}

async fn transcribe_with_whisper(audio_path: &str) -> Result<String, String> {
    let lang = crate::config::settings::resolve_whisper_language();
    crate::transcription::transcribe_audio_local(audio_path, lang.as_str(), "auto").await
}

/// STTプロバイダ設定に応じて文字起こしを実行する。
///
/// - Gemini: クラウドAPIへ送信
/// - Whisper: ローカルの whisper-cli で処理
/// - Hybrid: 録音時間が閾値以下なら Whisper、超えたら Gemini
/// - Collaborate: Whisper で文字起こし後、Gemini で整形。
///   整形に失敗しても Whisper 結果をそのまま返す（整形失敗は致命ではない）。
pub async fn transcribe_with_provider(
    provider: SttProvider,
    audio_path: &str,
    recording_duration_ms: Option<u64>,
) -> DispatchOutcome {
    let mut whisper_text: Option<String> = None;
    let mut deferred_error_logs: Vec<(ErrorType, String, String)> = Vec::new();

    let result = match provider {
        SttProvider::Gemini => transcribe_with_gemini(audio_path).await,
        SttProvider::Whisper => transcribe_with_whisper(audio_path).await,
        SttProvider::Hybrid => match recording_duration_ms {
            Some(duration_ms) => {
                let threshold_ms = crate::config::settings::get_hybrid_threshold_ms();
                if duration_ms <= threshold_ms {
                    info!(
                        "Hybrid判定: {}ms <= {}ms なので Whisper で処理",
                        duration_ms, threshold_ms
                    );
                    transcribe_with_whisper(audio_path).await
                } else {
                    info!(
                        "Hybrid判定: {}ms > {}ms なので Gemini で処理",
                        duration_ms, threshold_ms
                    );
                    transcribe_with_gemini(audio_path).await
                }
            }
            None => Err("Hybrid判定に必要な録音時間を取得できませんでした".to_string()),
        },
        SttProvider::Collaborate => match transcribe_with_whisper(audio_path).await {
            Ok(raw_text) => {
                whisper_text = Some(raw_text.clone());
                let model = crate::config::get_model();
                let polish_result = match crate::config::get_api_key() {
                    Ok(api_key) => crate::transcription::polish_text_with_model(
                        &raw_text, &api_key, &model,
                    )
                    .await
                    .map_err(|e| e.to_string()),
                    Err(e) => Err(format!("APIキーの取得に失敗: {}", e)),
                };

                match polish_result {
                    Ok(polished_text) => Ok(polished_text),
                    Err(e) => {
                        deferred_error_logs.push((
                            ErrorType::ApiError,
                            format!("Gemini整形に失敗したためWhisper結果を使用: {}", e),
                            format!(
                                "provider=collaborate, fallback=whisper, model={}, file={}",
                                model, audio_path
                            ),
                        ));
                        warn!("Collaborate整形失敗。Whisper結果を使用します: {}", e);
                        Ok(raw_text)
                    }
                }
            }
            Err(e) => Err(e),
        },
    };

    DispatchOutcome {
        result,
        whisper_text,
        deferred_error_logs,
    }
}
