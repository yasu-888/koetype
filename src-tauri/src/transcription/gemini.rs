use base64::{engine::general_purpose, Engine as _};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;
use thiserror::Error;
use tracing::{debug, error};

#[derive(Error, Debug)]
pub enum TranscriptionError {
    #[error("音声ファイルの読み込みに失敗しました: {0}")]
    FileRead(String),

    #[error("API呼び出しに失敗しました: {0}")]
    ApiCall(String),

    #[error("レスポンスの解析に失敗しました: {0}")]
    ParseResponse(String),

    #[error("文字起こし結果が空です")]
    EmptyResult,
}

#[derive(Serialize)]
struct GeminiRequest {
    contents: Vec<Content>,
}

#[derive(Serialize)]
struct Content {
    parts: Vec<Part>,
}

#[derive(Serialize)]
#[serde(untagged)]
enum Part {
    Text { text: String },
    InlineData { inline_data: InlineData },
}

#[derive(Serialize)]
struct InlineData {
    mime_type: String,
    data: String,
}

#[derive(Deserialize, Debug)]
struct GeminiResponse {
    candidates: Option<Vec<Candidate>>,
}

#[derive(Deserialize, Debug)]
struct Candidate {
    content: ResponseContent,
}

#[derive(Deserialize, Debug)]
struct ResponseContent {
    parts: Vec<ResponsePart>,
}

#[derive(Deserialize, Debug)]
struct ResponsePart {
    text: String,
}

/// Gemini APIを呼び出してレスポンスを返す共通ヘルパー
async fn call_gemini_api(
    api_key: &str,
    model: &str,
    request_body: &GeminiRequest,
) -> Result<GeminiResponse, TranscriptionError> {
    let client = Client::new();
    let url = format!(
        "https://generativelanguage.googleapis.com/v1beta/models/{}:generateContent",
        model
    );

    debug!("Gemini APIを呼び出します: {}", url);

    let response = client
        .post(&url)
        .header("x-goog-api-key", api_key)
        .header("Content-Type", "application/json")
        .json(request_body)
        .send()
        .await
        .map_err(|e| TranscriptionError::ApiCall(format!("リクエストエラー: {}", e)))?;

    if !response.status().is_success() {
        let status = response.status();
        let error_text = response
            .text()
            .await
            .unwrap_or_else(|_| "レスポンス取得失敗".to_string());
        error!("API エラー: {} - {}", status, error_text);
        return Err(TranscriptionError::ApiCall(format!(
            "HTTPエラー {}: {}",
            status, error_text
        )));
    }

    response
        .json()
        .await
        .map_err(|e| TranscriptionError::ParseResponse(format!("JSONパースエラー: {}", e)))
}

/// レスポンスからテキストを抽出する共通ヘルパー
fn extract_text_from_response(response: GeminiResponse) -> Result<String, TranscriptionError> {
    response
        .candidates
        .and_then(|candidates| candidates.into_iter().next())
        .and_then(|candidate| {
            candidate
                .content
                .parts
                .into_iter()
                .map(|part| part.text)
                .collect::<Vec<_>>()
                .into_iter()
                .next()
        })
        .ok_or(TranscriptionError::EmptyResult)
}

/// ユーザー辞書プロンプト文字列を構築する共通ヘルパー
/// recognition_message: 辞書ヒントの前置きメッセージ
fn build_dictionary_prompt(recognition_message: &str) -> String {
    let dictionary = crate::config::settings::get_user_dictionary();
    if dictionary.is_empty() {
        return String::new();
    }
    let words = dictionary
        .iter()
        .map(|e| format!("- {}", e.word))
        .collect::<Vec<_>>()
        .join("\n");
    format!("\n{}:\n{}\n", recognition_message, words)
}

/// Gemini APIを使って音声ファイルを文字起こし（モデル指定可能）
pub async fn transcribe_audio_with_model(
    file_path: &str,
    api_key: &str,
    model: &str,
) -> Result<String, TranscriptionError> {
    debug!("文字起こし開始: {} (モデル: {})", file_path, model);

    let audio_data = fs::read(Path::new(file_path))
        .map_err(|e| TranscriptionError::FileRead(format!("ファイル読み込みエラー: {}", e)))?;
    let audio_base64 = general_purpose::STANDARD.encode(&audio_data);

    let dictionary_prompt = build_dictionary_prompt("以下の単語は正確に認識してください");

    let prompt = format!(
        "音声を忠実に日本語で文字起こししてください。{}

以下のルールを厳守してください：

## 必須事項
1. タイムスタンプ（00:01など）は絶対に含めないでください
2. 話者識別（話者A：など）は含めないでください
3. 解説やメタデータを含めず、音声内の言葉のみを出力してください
4. 出力は純粋なテキストのみとしてください
5. 口調（丁寧語への変換、語尾の変更、タメ口から敬語への変換など）は一切行わず、話している通りに出力してください。独り言やラフな口調もそのまま維持してください。

## 文章の整形
6. フィラー（「えー」「あのー」など）は削除してください
7. 句読点（、。）を適切に挿入してください
8. 並列表現には中黒（・）を使用してください（例：リンゴ・バナナ・オレンジ）
9. 意味のまとまりごとに改行を入れてください
10. 重複や言い間違いは、明らかに不要なミスと思われるもののみ削除してください。

ビジネス文書風に整えたりせず、話者の言葉をそのまま出力してください。",
        dictionary_prompt
    );

    let request_body = GeminiRequest {
        contents: vec![Content {
            parts: vec![
                Part::Text { text: prompt },
                Part::InlineData {
                    inline_data: InlineData {
                        mime_type: "audio/wav".to_string(),
                        data: audio_base64,
                    },
                },
            ],
        }],
    };

    let response = call_gemini_api(api_key, model, &request_body).await?;
    let text = extract_text_from_response(response)?;

    debug!("文字起こし完了: {} 文字", text.len());
    Ok(text.trim().to_string())
}

/// Whisperなどで得たテキストをGeminiで整形する
pub async fn polish_text_with_model(
    input_text: &str,
    api_key: &str,
    model: &str,
) -> Result<String, TranscriptionError> {
    debug!(
        "テキスト整形開始: {} 文字 (モデル: {})",
        input_text.len(),
        model
    );

    let trimmed_input = input_text.trim();
    if trimmed_input.is_empty() {
        return Err(TranscriptionError::EmptyResult);
    }

    let dictionary_prompt = build_dictionary_prompt("以下の単語は表記を維持してください");

    let prompt = format!(
        "以下の文字起こしテキストについて、誤字脱字や明らかな入力ミスの修正のみを行ってください。{}

以下のルールを厳守してください：
1. 事実や内容を追加しない
2. 意味を変更しない
3. 口調（丁寧語への変換、語尾の変更、スタイルなど）は一切変更せず、入力テキストの雰囲気を完全に維持してください。
4. フィラーや、明らかに不要だと思われる重複のみを削減してください。
5. 句読点と改行を補って可読性を上げてください。
6. 出力は修正後の本文のみとしてください（説明や注釈は一切不要）

--- 入力テキスト ---
{}
--- 入力テキストここまで ---",
        dictionary_prompt, trimmed_input
    );

    let request_body = GeminiRequest {
        contents: vec![Content {
            parts: vec![Part::Text { text: prompt }],
        }],
    };

    let response = call_gemini_api(api_key, model, &request_body).await?;
    let text = extract_text_from_response(response)?;

    let polished = text.trim().to_string();
    if polished.is_empty() {
        return Err(TranscriptionError::EmptyResult);
    }

    debug!("テキスト整形完了: {} 文字", polished.len());
    Ok(polished)
}

/// 選択テキストを音声指示で編集する
pub async fn process_selected_text_with_voice(
    selected_text: &str,
    voice_text: &str,
    api_key: &str,
    model: &str,
) -> Result<String, TranscriptionError> {
    let selected = selected_text.trim();
    let voice = voice_text.trim();
    if selected.is_empty() {
        return Err(TranscriptionError::EmptyResult);
    }
    if voice.is_empty() {
        return Err(TranscriptionError::EmptyResult);
    }

    let prompt = format!(
        "あなたはテキスト編集アシスタントです。選択テキストを音声指示に従って編集してください。

<requirements>
<rule>出力は編集後の本文のみ（説明・注釈・引用符なし）</rule>
<rule>音声指示に明示されない内容を勝手に追加しない</rule>
<rule>不明点は保守的に解釈し、原文を優先する</rule>
</requirements>

<selected_text>
{}
</selected_text>

<voice_instruction>
{}
</voice_instruction>",
        selected, voice
    );

    let request_body = GeminiRequest {
        contents: vec![Content {
            parts: vec![Part::Text { text: prompt }],
        }],
    };

    let response = call_gemini_api(api_key, model, &request_body).await?;
    let edited = extract_text_from_response(response)?.trim().to_string();

    if edited.is_empty() {
        return Err(TranscriptionError::EmptyResult);
    }

    Ok(edited)
}

/// Gemini API接続テスト（シンプルなテキスト生成）
pub async fn test_connection(api_key: &str, model: &str) -> Result<String, TranscriptionError> {
    debug!("Gemini API接続テスト開始 (モデル: {})", model);

    let request_body = GeminiRequest {
        contents: vec![Content {
            parts: vec![Part::Text {
                text: "こんにちは".to_string(),
            }],
        }],
    };

    let response = call_gemini_api(api_key, model, &request_body).await?;
    let text = extract_text_from_response(response)?;

    debug!("接続テスト成功: レスポンス受信");
    Ok(format!(
        "接続成功 - モデル: {}\nレスポンス: {}",
        model,
        text.trim()
    ))
}
