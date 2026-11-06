use crate::audio::models;
use crate::config::Config;
use anyhow::{Result, bail};
use clap::Subcommand;
use colored::*;

#[derive(Subcommand)]
pub enum ModelsAction {
    /// List installed and downloadable Whisper models (default)
    List {
        /// Fetch the live list of downloadable models (includes quantized variants)
        #[arg(long)]
        remote: bool,
    },

    /// Download a Whisper model (e.g. base.en, small, large-v3-turbo)
    Install {
        /// Model name, as in whisper.cpp's ggml-<name>.bin files
        name: String,
        /// Download again even if already installed
        #[arg(long)]
        force: bool,
        /// Also make it the model used for transcription
        #[arg(long)]
        select: bool,
    },

    /// Delete an installed model
    Remove {
        /// Model name
        name: String,
        /// Allow deleting the model currently used for transcription
        #[arg(long)]
        force: bool,
    },

    /// Use an installed model for transcription (sets audio.whisper_model)
    Use {
        /// Model name
        name: String,
    },

    /// Show the directory where models are stored
    Path,
}

pub fn handle_models_command(action: Option<ModelsAction>, config: &Config) -> Result<()> {
    match action.unwrap_or(ModelsAction::List { remote: false }) {
        ModelsAction::List { remote } => list_models(config, remote),
        ModelsAction::Install {
            name,
            force,
            select,
        } => {
            let name = models::normalize_name(&name)?;
            println!("{}", format!("Downloading model '{name}'...").cyan());
            let path = models::install(&name, force)?;
            println!(
                "{}",
                format!("✓ Installed '{name}' at {}", path.display()).green()
            );
            if select {
                select_model(config, &name)?;
            }
            Ok(())
        }
        ModelsAction::Remove { name, force } => {
            let name = models::normalize_name(&name)?;
            if is_active(config, &name) && !force {
                bail!(
                    "'{name}' is the model currently used for transcription; \
                     switch with `cl models use <other>` or pass --force"
                );
            }
            if !models::remove(&name)? {
                bail!("Model '{name}' is not installed");
            }
            println!("{}", format!("Model '{name}' removed").green());
            Ok(())
        }
        ModelsAction::Use { name } => {
            let name = models::normalize_name(&name)?;
            if !models::model_path(&name)?.exists() {
                bail!("Model '{name}' is not installed; run `cl models install {name}` first");
            }
            select_model(config, &name)
        }
        ModelsAction::Path => {
            println!("{}", models::models_dir()?.display());
            Ok(())
        }
    }
}

fn is_active(config: &Config, name: &str) -> bool {
    models::normalize_name(&config.audio.whisper_model).is_ok_and(|active| active == name)
}

fn select_model(config: &Config, name: &str) -> Result<()> {
    let mut new_config = config.clone();
    new_config.set("audio.whisper_model", name)?;
    new_config.save()?;
    println!("{}", format!("Transcription now uses '{name}'").green());
    Ok(())
}

fn list_models(config: &Config, remote: bool) -> Result<()> {
    let installed = models::installed()?;

    println!("{}", "Installed models:".cyan().bold());
    if installed.is_empty() {
        println!(
            "  {}",
            "none (try `cl models install base.en`)".bright_black()
        );
    }
    for model in &installed {
        let marker = if is_active(config, &model.name) {
            " (active)".green().to_string()
        } else {
            String::new()
        };
        println!(
            "  {}  {}{}",
            format!("{:<18}", model.name).bold(),
            models::format_size(model.size_bytes),
            marker
        );
    }

    let active = models::normalize_name(&config.audio.whisper_model).ok();
    if let Some(active) = &active
        && !installed.iter().any(|m| &m.name == active)
    {
        println!(
            "  {}",
            format!("active model '{active}' is not installed").yellow()
        );
    }

    // (name, size text) for what can still be downloaded
    let mut available: Vec<(String, String)> = Vec::new();
    let mut note = "Other whisper.cpp models (e.g. quantized variants) can be installed by name; \
                    `cl models list --remote` shows them all."
        .to_string();
    if remote {
        match models::fetch_remote() {
            Ok(list) => {
                available = list
                    .into_iter()
                    .map(|m| (m.name, models::format_size(m.size_bytes)))
                    .collect();
                note = String::new();
            }
            Err(e) => {
                println!();
                println!(
                    "{}",
                    format!("Could not fetch the live list: {e:#}").yellow()
                );
                println!("{}", "Showing the built-in list instead.".yellow());
            }
        }
    }
    if available.is_empty() {
        available = models::KNOWN_MODELS
            .iter()
            .map(|(name, size)| (name.to_string(), size.to_string()))
            .collect();
    }

    println!();
    println!("{}", "Available to install:".cyan().bold());
    for (name, size) in available {
        if installed.iter().any(|m| m.name == name) {
            continue;
        }
        println!("  {:<22}  {}", name, size.bright_black());
    }
    if !note.is_empty() {
        println!("  {}", note.bright_black());
    }
    Ok(())
}
