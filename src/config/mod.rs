use anyhow::{Context, Result};
use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub database: DatabaseConfig,
    pub editor: EditorConfig,
    pub display: DisplayConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseConfig {
    pub path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EditorConfig {
    pub command: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DisplayConfig {
    pub colors_enabled: bool,
    pub date_format: String,
    pub entries_per_page: Option<usize>,

    #[serde(default)]
    pub stardate_mode: bool,
}

impl Default for Config {
    fn default() -> Self {
        // Find default project directories

        Config {
            database: DatabaseConfig { path: None },
            editor: EditorConfig {
                command: Some("vim".into()),
            },
            display: DisplayConfig {
                colors_enabled: true,
                date_format: "%Y-%m-%d %H:%M:%S".to_string(),
                entries_per_page: None,
                stardate_mode: false,
            },
        }
    }
}

impl Config {
    pub fn load() -> Result<Self> {
        let config_path = Self::get_config_path()?;

        if config_path.exists() {
            let content = fs::read_to_string(&config_path)
                .with_context(|| format!("Failed to read config file at {:?}", config_path))?;

            let config: Config = serde_json::from_str(&content)
                .with_context(|| "Failed to parse config file as JSON")?;

            Ok(config)
        } else {
            let config = Config::default();
            config.save()?;
            Ok(config)
        }
    }

    pub fn save(&self) -> Result<()> {
        let config_path = Self::get_config_path()?;

        if let Some(parent) = config_path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("Failed to create config directory {:?}", parent))?;
        }

        let content =
            serde_json::to_string_pretty(self).context("Failed to serialize config to JSON")?;

        fs::write(&config_path, content)
            .with_context(|| format!("Failed to write config file at {:?}", config_path))?;

        Ok(())
    }

    pub fn get_config_path() -> Result<PathBuf> {
        let proj_dirs = ProjectDirs::from("", "", "captains-log")
            .context("Failed to get project directories")?;

        Ok(proj_dirs.config_dir().join("config.json"))
    }

    pub fn get_database_path(&self) -> Result<PathBuf> {
        if let Some(custom_path) = &self.database.path {
            Ok(PathBuf::from(custom_path))
        } else {
            let proj_dirs = ProjectDirs::from("", "", "captains-log")
                .context("Failed to get project directories")?;
            Ok(proj_dirs.data_dir().join("journal.db"))
        }
    }

    pub fn get_editor_command(&self) -> String {
        if let Some(command) = &self.editor.command {
            command.clone()
        } else {
            std::env::var("EDITOR").unwrap_or_else(|_| "vim".to_string())
        }
    }
}
