use anyhow::{Context, Result};
use chrono::Local;
use rand::distr::SampleString;
use std::fs;
use std::path::{Path, PathBuf};

/// Get the audio directory path (alongside the database)
pub fn get_audio_directory(db_path: &Path) -> Result<PathBuf> {
    let db_dir = db_path
        .parent()
        .context("Failed to get database directory")?;
    Ok(db_dir.join("audio"))
}

/// Generate a unique audio filename with timestamp and random suffix
/// Format: YYYYMMDD_HHMMSS_random6.wav
pub fn generate_audio_filename() -> String {
    let now = Local::now();
    let timestamp = now.format("%Y%m%d_%H%M%S");

    // Generate 6 random alphanumeric characters
    let suffix = rand::distr::Alphanumeric.sample_string(&mut rand::rng(), 6);

    format!("{}_{}.wav", timestamp, suffix.to_lowercase())
}

/// Ensure the audio directory exists, creating it if necessary
pub fn ensure_audio_directory_exists(db_path: &Path) -> Result<()> {
    let audio_dir = get_audio_directory(db_path)?;

    if !audio_dir.exists() {
        fs::create_dir_all(&audio_dir)
            .with_context(|| format!("Failed to create audio directory at {:?}", audio_dir))?;
    }

    Ok(())
}

/// Get the full absolute path for an audio file given its relative path
pub fn get_audio_full_path(db_path: &Path, relative_path: &str) -> Result<PathBuf> {
    let db_dir = db_path
        .parent()
        .context("Failed to get database directory")?;
    Ok(db_dir.join(relative_path))
}
