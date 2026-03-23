use crate::config::Config;
use anyhow::{Context, Result};
use colored::Colorize;
use directories::ProjectDirs;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Transcribe audio file using Whisper
pub fn transcribe_audio(config: &Config, audio_path: &Path) -> Result<String> {
    println!("{}", "🔄 Transcribing audio...".cyan());

    // Detect whisper binary
    let whisper_cmd = config
        .audio
        .whisper_command
        .clone()
        .or_else(|| detect_whisper().ok())
        .context(create_whisper_not_found_error())?;

    // Get model path
    let model_path = get_model_path(&config.audio.whisper_model)?;

    // Verify model exists
    if !model_path.exists() {
        return Err(create_model_not_found_error(&config.audio.whisper_model));
    }

    // Run transcription
    let output = run_whisper(&whisper_cmd, &model_path, audio_path)?;

    println!("{}", "✓ Transcription complete".green());

    Ok(output)
}

/// Detect whisper.cpp binary
pub fn detect_whisper() -> Result<String> {
    // Try common whisper.cpp binary names
    // whisper-cli is the Homebrew package name
    let whisper_names = vec!["whisper-cli", "whisper-cpp", "whisper", "main"];

    for name in &whisper_names {
        if let Ok(path) = which::which(name) {
            return Ok(path.to_string_lossy().to_string());
        }
    }

    Err(anyhow::anyhow!("Whisper binary not found"))
}

/// Get the path to the Whisper model
fn get_model_path(model_name: &str) -> Result<PathBuf> {
    let proj_dirs =
        ProjectDirs::from("", "", "captains-log").context("Failed to get project directories")?;

    let models_dir = proj_dirs.data_dir().join("models");

    // Ensure models directory exists
    if !models_dir.exists() {
        fs::create_dir_all(&models_dir)
            .with_context(|| format!("Failed to create models directory at {:?}", models_dir))?;
    }

    // Construct model filename
    let model_filename = if model_name.starts_with("ggml-") {
        model_name.to_string()
    } else {
        format!("ggml-{}.bin", model_name)
    };

    Ok(models_dir.join(model_filename))
}

/// Run whisper transcription
fn run_whisper(whisper_cmd: &str, model_path: &Path, audio_path: &Path) -> Result<String> {
    let model_str = model_path.to_str().context("Invalid model path")?;
    let audio_str = audio_path.to_str().context("Invalid audio path")?;

    let output = Command::new(whisper_cmd)
        .args(&[
            "-m", model_str, // Model file
            "-nt",     // No timestamps in output
            audio_str, // Audio file (positional argument)
        ])
        .output()
        .context("Failed to execute whisper")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(anyhow::anyhow!("Whisper transcription failed: {}", stderr));
    }

    // Parse output - whisper.cpp writes transcription to stdout
    let transcription =
        String::from_utf8(output.stdout).context("Failed to parse whisper output as UTF-8")?;

    // Clean up the transcription
    let cleaned = transcription
        .lines()
        .filter(|line| {
            // Filter out whisper.cpp's log lines
            !line.starts_with("whisper_")
                && !line.contains("processing")
                && !line.is_empty()
                && !line.trim().starts_with('[')
        })
        .map(|line| line.trim())
        .collect::<Vec<&str>>()
        .join(" ");

    if cleaned.is_empty() {
        return Err(anyhow::anyhow!(
            "Transcription produced no output - audio may be silent or corrupted"
        ));
    }

    Ok(cleaned)
}

fn create_whisper_not_found_error() -> anyhow::Error {
    let msg = format!("{}", "Error: Whisper binary not found".red().bold());
    let help = format!(
        "Please install whisper.cpp:\n\n\
        {}:\n  \
        git clone https://github.com/ggerganov/whisper.cpp\n  \
        cd whisper.cpp\n  \
        make\n  \
        # Add the binary to PATH or configure with:\n  \
        cl config set audio.whisper_command \"/path/to/whisper.cpp/main\"\n\n\
        {}:\n  \
        brew install whisper-cpp\n\n\
        For more info: {}",
        "Build from source".green(),
        "macOS (Homebrew)".green(),
        "https://github.com/ggerganov/whisper.cpp".blue()
    );

    anyhow::anyhow!("{}\n{}", msg, help)
}

fn create_model_not_found_error(model_name: &str) -> anyhow::Error {
    let proj_dirs =
        ProjectDirs::from("", "", "captains-log").expect("Failed to get project directories");
    let models_dir = proj_dirs.data_dir().join("models");

    let model_filename = if model_name.starts_with("ggml-") {
        model_name.to_string()
    } else {
        format!("ggml-{}.bin", model_name)
    };

    let msg = format!("{}", "Error: Whisper model not found".red().bold());
    let help = format!(
        "Model '{}' not found at: {}\n\n\
        {}:\n  \
        # Download the model from whisper.cpp repository\n  \
        cd {}\n  \
        curl -L -o {} https://huggingface.co/ggerganov/whisper.cpp/resolve/main/{}\n\n\
        {}:\n  \
        tiny.en (~75MB) - Fastest, least accurate\n  \
        base.en (~140MB) - Good balance (recommended)\n  \
        small.en (~460MB) - Better accuracy\n  \
        medium.en (~1.5GB) - High accuracy\n\n\
        For more models: {}",
        model_name,
        models_dir.join(&model_filename).display(),
        "Download instructions".green(),
        models_dir.display(),
        model_filename,
        model_filename,
        "Available models".green(),
        "https://huggingface.co/ggerganov/whisper.cpp".blue()
    );

    anyhow::anyhow!("{}\n{}", msg, help)
}
