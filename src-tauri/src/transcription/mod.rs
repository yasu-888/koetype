pub mod dispatch;
pub mod gemini;
pub mod whisper_local;

pub use gemini::{
    polish_text_with_model, process_selected_text_with_voice, test_connection,
    transcribe_audio_with_model,
};
pub use whisper_local::transcribe_audio_local;
