mod platform;
mod playback;
mod recording;
mod storage;
mod transcription;

// Platform module is used internally by recording and playback modules
pub use playback::play_audio;
pub use recording::record_audio;
pub use storage::{
    ensure_audio_directory_exists, generate_audio_filename, get_audio_directory,
    get_audio_full_path,
};
pub use transcription::transcribe_audio;
