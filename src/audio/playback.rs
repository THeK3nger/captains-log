use crate::audio::platform::{detect_platform, detect_playback_tool};
use crate::config::Config;
use anyhow::{Context, Result};
use std::path::Path;
use std::process::Command;

/// Play audio file at the specified path
pub fn play_audio(config: &Config, audio_path: &Path) -> Result<()> {
    // Get playback tool
    let tool = config
        .audio
        .playback_tool
        .clone()
        .or_else(|| detect_playback_tool().ok())
        .context("No playback tool available")?;

    // Build platform-specific playback command
    let mut cmd = build_playback_command(&tool, audio_path)?;

    // Execute playback (blocking)
    let status = cmd.status().context("Failed to execute playback")?;

    if !status.success() {
        return Err(anyhow::anyhow!("Playback failed with status: {}", status));
    }

    Ok(())
}

/// Build the playback command based on tool and platform
fn build_playback_command(tool: &str, audio_path: &Path) -> Result<Command> {
    let platform = detect_platform();
    let audio_str = audio_path.to_str().context("Invalid audio path")?;

    let mut cmd = Command::new(tool);

    match tool {
        "afplay" => {
            // macOS default audio player
            cmd.arg(audio_str);
        }
        "ffplay" => {
            // FFmpeg audio player
            cmd.args(&[
                "-nodisp",   // No video display
                "-autoexit", // Exit when done
                audio_str,
            ]);
        }
        "aplay" => {
            // ALSA player (Linux)
            cmd.arg(audio_str);
        }
        "play" => {
            // SoX player
            cmd.arg(audio_str);
        }
        _ => {
            return Err(anyhow::anyhow!(
                "Unsupported playback tool: {} for platform: {:?}",
                tool,
                platform
            ));
        }
    }

    Ok(cmd)
}

/// Get audio duration in seconds (optional utility function)
/// Returns None if duration cannot be determined
#[allow(dead_code)]
pub fn get_audio_duration(audio_path: &Path) -> Option<f64> {
    // Try using ffprobe if available
    if which::which("ffprobe").is_ok() {
        let output = Command::new("ffprobe")
            .args(&[
                "-v",
                "error",
                "-show_entries",
                "format=duration",
                "-of",
                "default=noprint_wrappers=1:nokey=1",
                audio_path.to_str()?,
            ])
            .output()
            .ok()?;

        if output.status.success() {
            let duration_str = String::from_utf8(output.stdout).ok()?;
            return duration_str.trim().parse::<f64>().ok();
        }
    }

    None
}
