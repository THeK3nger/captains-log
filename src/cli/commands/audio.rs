use crate::config::Config;
use crate::journal::{Journal, NewEntry};
use anyhow::Result;
use colored::*;

pub fn handle_record_command(
    journal_obj: &Journal,
    config: &Config,
    db_path: &std::path::Path,
    journal_category: Option<String>,
    no_transcribe: bool,
    max_duration: Option<u64>,
) -> Result<()> {
    use crate::audio::{
        ensure_audio_directory_exists, generate_audio_filename, get_audio_directory, record_audio,
        transcribe_audio,
    };

    // Ensure audio directory exists
    ensure_audio_directory_exists(db_path)?;

    // Generate filename
    let audio_dir = get_audio_directory(db_path)?;
    let filename = generate_audio_filename();
    let full_path = audio_dir.join(&filename);

    // Get max duration from config or parameter
    let max_duration_secs = max_duration.unwrap_or(config.audio.max_recording_seconds);

    // Record audio
    let duration = record_audio(config, &full_path, max_duration_secs)?;

    // Transcribe audio (unless skipped)
    let transcription = if no_transcribe {
        println!(
            "{}",
            "Skipping transcription (--no-transcribe flag)".yellow()
        );
        "[Audio entry - no transcription]".to_string()
    } else {
        match transcribe_audio(config, &full_path) {
            Ok(text) => {
                // Display transcription to user
                println!();
                println!("{}", "─── Transcription ───".cyan().bold());
                println!("{}", text);
                println!("{}", "─────────────────────".cyan().bold());
                println!();
                text
            }
            Err(e) => {
                println!(
                    "{}",
                    format!("Warning: Transcription failed: {}", e).yellow()
                );
                println!("{}", "Saving entry with audio only...".yellow());
                "[Transcription failed - audio only]".to_string()
            }
        }
    };

    println!("{}", "📝 Creating journal entry...".cyan());

    // Create entry with audio
    // Store relative path: audio/filename.wav
    let relative_path = format!("audio/{}", filename);

    let entry_id = journal_obj.create_entry(
        NewEntry::new(&transcription)
            .journal(journal_category.as_deref())
            .audio_path(Some(&relative_path)),
    )?;

    println!(
        "{}",
        format!(
            "✓ Entry {} created successfully with audio attached",
            entry_id
        )
        .green()
    );
    println!("  {}: {:.1}s", "Duration".cyan(), duration.as_secs_f64());
    println!("  {}: {}", "Audio".cyan(), relative_path.green());

    Ok(())
}

pub fn handle_play_command(
    journal_obj: &Journal,
    config: &Config,
    db_path: &std::path::Path,
    id: i64,
) -> Result<()> {
    use crate::audio::{get_audio_full_path, play_audio};
    use colored::Colorize;

    // Get entry
    let entry = journal_obj
        .get_entry(id)?
        .ok_or_else(|| anyhow::anyhow!("Entry {} not found", id))?;

    // Check if entry has audio
    let audio_path = entry
        .audio_path
        .ok_or_else(|| anyhow::anyhow!("Entry {} has no audio recording", id))?;

    // Get full path
    let full_path = get_audio_full_path(db_path, &audio_path)?;

    // Check if file exists
    if !full_path.exists() {
        return Err(anyhow::anyhow!(
            "Audio file not found at {}. It may have been moved or deleted.",
            full_path.display()
        ));
    }

    println!(
        "{}",
        format!("🎵 Playing audio from entry {}...", id).cyan()
    );
    println!("  {}: {}", "Audio".cyan(), audio_path.green());

    // Play audio
    play_audio(config, &full_path)?;

    println!("{}", "✓ Playback complete".green());

    Ok(())
}
