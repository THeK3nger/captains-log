use crate::config::{Config, DEFAULT_LIST_LIMIT};
use anyhow::Result;
use clap::Subcommand;
use colored::*;

#[derive(Subcommand)]
pub enum ConfigAction {
    /// Show current configuration
    Show,
    /// Set a configuration value
    Set {
        /// Configuration key (e.g., editor.command, database.path)
        key: String,
        /// Configuration value
        value: String,
    },
    /// Show configuration file path
    Path,
}

pub fn handle_config_command(action: Option<ConfigAction>, config: &Config) -> Result<()> {
    match action {
        Some(ConfigAction::Show) | None => show_config(config),
        Some(ConfigAction::Set { key, value }) => set_config(config, &key, &value)?,
        Some(ConfigAction::Path) => println!("{}", Config::get_config_path()?.display()),
    }

    Ok(())
}

fn set_config(config: &Config, key: &str, value: &str) -> Result<()> {
    let mut new_config = config.clone();
    let shown = new_config.set(key, value)?;
    println!("{}", format!("Set {key} to {shown}").green());

    new_config.save()?;
    println!(
        "{}",
        "Configuration saved successfully".bright_green().bold()
    );
    Ok(())
}

fn show_config(config: &Config) {
    println!("{}", "Current Configuration:".cyan().bold());
    println!("{}", "─".repeat(40).bright_blue());

    println!();
    println!("{}", "Database:".yellow().bold());
    if let Some(path) = &config.database.path {
        println!("  path: {}", path.green());
    } else {
        println!("  path: {} (default)", "auto".bright_black());
    }

    println!();
    println!("{}", "Editor:".yellow().bold());
    if let Some(command) = &config.editor.command {
        println!("  command: {}", command.green());
    } else {
        println!(
            "  command: {} (from $EDITOR or default)",
            "auto".bright_black()
        );
    }

    println!();
    println!("{}", "Display:".yellow().bold());
    println!(
        "  colors_enabled: {}",
        config.display.colors_enabled.to_string().green()
    );
    println!("  date_format: {}", config.display.date_format.green());
    println!(
        "  stardate_mode: {}",
        config.display.stardate_mode.to_string().green()
    );
    if let Some(entries_per_page) = config.display.entries_per_page {
        println!(
            "  entries_per_page: {}",
            entries_per_page.to_string().green()
        );
    } else {
        println!(
            "  entries_per_page: {} (default: {})",
            "auto".bright_black(),
            DEFAULT_LIST_LIMIT
        );
    }
    if let Some(tz) = &config.display.timezone {
        println!("  timezone: {}", tz.green());
    } else {
        println!("  timezone: {} (system local time)", "auto".bright_black());
    }

    println!();
    println!("{}", "Audio:".yellow().bold());
    if let Some(whisper_command) = &config.audio.whisper_command {
        println!("  whisper_command: {}", whisper_command.green());
    } else {
        println!("  whisper_command: {} (auto-detect)", "auto".bright_black());
    }
    println!("  whisper_model: {}", config.audio.whisper_model.green());
    if let Some(recording_tool) = &config.audio.recording_tool {
        println!("  recording_tool: {}", recording_tool.green());
    } else {
        println!("  recording_tool: {} (auto-detect)", "auto".bright_black());
    }
    if let Some(playback_tool) = &config.audio.playback_tool {
        println!("  playback_tool: {}", playback_tool.green());
    } else {
        println!("  playback_tool: {} (auto-detect)", "auto".bright_black());
    }
    println!(
        "  max_recording_seconds: {}",
        config.audio.max_recording_seconds.to_string().green()
    );
    println!(
        "  sample_rate: {}",
        config.audio.sample_rate.to_string().green()
    );
}
