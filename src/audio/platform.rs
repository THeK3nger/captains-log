use anyhow::{anyhow, Result};
use colored::Colorize;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Platform {
    MacOS,
    Linux,
    Windows,
}

/// Detect the current platform
pub fn detect_platform() -> Platform {
    match std::env::consts::OS {
        "macos" => Platform::MacOS,
        "linux" => Platform::Linux,
        "windows" => Platform::Windows,
        _ => Platform::Linux, // Default fallback
    }
}

/// Detect available recording tool for the current platform
pub fn detect_recording_tool() -> Result<String> {
    let platform = detect_platform();

    // Priority order based on platform
    let tools = match platform {
        Platform::MacOS => vec!["sox", "ffmpeg"],
        Platform::Linux => vec!["arecord", "sox", "ffmpeg"],
        Platform::Windows => vec!["ffmpeg"],
    };

    // Try to find the first available tool
    for tool in &tools {
        if which::which(tool).is_ok() {
            return Ok(tool.to_string());
        }
    }

    // No tool found - provide helpful error message
    Err(create_recording_tool_error(platform))
}

/// Detect available playback tool for the current platform
pub fn detect_playback_tool() -> Result<String> {
    let platform = detect_platform();

    // Priority order based on platform
    let tools = match platform {
        Platform::MacOS => vec!["afplay", "ffplay", "play"],
        Platform::Linux => vec!["ffplay", "aplay", "play"],
        Platform::Windows => vec!["ffplay"],
    };

    // Try to find the first available tool
    for tool in &tools {
        if which::which(tool).is_ok() {
            return Ok(tool.to_string());
        }
    }

    // No tool found - provide helpful error message
    Err(create_playback_tool_error(platform))
}

fn create_recording_tool_error(platform: Platform) -> anyhow::Error {
    let msg = format!("{}", "Error: No recording tool found".red().bold());
    let help = match platform {
        Platform::MacOS => {
            format!(
                "Please install one of the following:\n  • {}\n  • {}",
                "brew install sox".green(),
                "brew install ffmpeg".green()
            )
        }
        Platform::Linux => {
            format!(
                "Please install one of the following:\n  • {} (ALSA utils)\n  • {}\n  • {}",
                "sudo apt install alsa-utils".green(),
                "sudo apt install sox".green(),
                "sudo apt install ffmpeg".green()
            )
        }
        Platform::Windows => {
            format!(
                "Please install:\n  • {}",
                "choco install ffmpeg".green()
            )
        }
    };

    anyhow!("{}\n{}", msg, help)
}

fn create_playback_tool_error(platform: Platform) -> anyhow::Error {
    let msg = format!("{}", "Error: No playback tool found".red().bold());
    let help = match platform {
        Platform::MacOS => {
            "afplay should be available by default on macOS.\nIf not, install: brew install ffmpeg"
                .to_string()
        }
        Platform::Linux => {
            format!(
                "Please install one of the following:\n  • {} (FFmpeg)\n  • {} (ALSA utils)\n  • {} (SoX)",
                "sudo apt install ffmpeg".green(),
                "sudo apt install alsa-utils".green(),
                "sudo apt install sox".green()
            )
        }
        Platform::Windows => {
            format!(
                "Please install:\n  • {}",
                "choco install ffmpeg".green()
            )
        }
    };

    anyhow!("{}\n{}", msg, help)
}
