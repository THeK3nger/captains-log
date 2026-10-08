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
    #[serde(default)]
    pub audio: AudioConfig,
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

    /// IANA timezone name for export timestamps (e.g. "Europe/Rome").
    /// If absent, the system local time is used.
    #[serde(default)]
    pub timezone: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioConfig {
    #[serde(default)]
    pub whisper_command: Option<String>,

    #[serde(default = "default_whisper_model")]
    pub whisper_model: String,

    #[serde(default)]
    pub recording_tool: Option<String>,

    #[serde(default)]
    pub playback_tool: Option<String>,

    #[serde(default = "default_max_recording_seconds")]
    pub max_recording_seconds: u64,

    #[serde(default = "default_sample_rate")]
    pub sample_rate: u32,
}

fn default_whisper_model() -> String {
    "base.en".to_string()
}

fn default_max_recording_seconds() -> u64 {
    600 // 10 minutes
}

fn default_sample_rate() -> u32 {
    16000
}

impl Default for AudioConfig {
    fn default() -> Self {
        AudioConfig {
            whisper_command: None,
            whisper_model: default_whisper_model(),
            recording_tool: None,
            playback_tool: None,
            max_recording_seconds: default_max_recording_seconds(),
            sample_rate: default_sample_rate(),
        }
    }
}

impl Default for Config {
    fn default() -> Self {
        // Find default project directories

        Config {
            database: DatabaseConfig { path: None },
            editor: EditorConfig { command: None },
            display: DisplayConfig {
                colors_enabled: true,
                date_format: "%Y-%m-%d %H:%M:%S".to_string(),
                entries_per_page: Some(20),
                stardate_mode: false,
                timezone: None,
            },
            audio: AudioConfig::default(),
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

    /// Configured display timezone, or `None` for the system local timezone.
    pub fn get_timezone(&self) -> Option<chrono_tz::Tz> {
        self.display
            .timezone
            .as_deref()
            .and_then(|tz| tz.parse().ok())
    }

    pub fn get_editor_command(&self) -> String {
        self.editor
            .command
            .clone()
            .or_else(|| std::env::var("EDITOR").ok())
            .unwrap_or_else(|| "vim".to_string())
    }
}

/// Default number of entries `list` shows when `display.entries_per_page` is unset.
pub const DEFAULT_LIST_LIMIT: usize = 20;

/// A setting that can be changed with `cl config set <key> <value>`.
pub struct Setting {
    pub key: &'static str,
    /// Validate `value` and apply it to the config. Returns how the new value
    /// should be shown to the user (e.g. `'vim'`, `true`, `auto`).
    apply: fn(&mut Config, &str) -> Result<String>,
}

fn parse_bool(key: &str, value: &str) -> Result<bool> {
    value
        .parse()
        .with_context(|| format!("{key} must be 'true' or 'false'"))
}

fn is_auto(value: &str) -> bool {
    value == "auto" || value == "none"
}

/// Every settable key, in the order they are listed in error messages. This is
/// the single source of truth for which keys exist.
pub const SETTINGS: &[Setting] = &[
    Setting {
        key: "database.path",
        apply: |config, value| {
            config.database.path = Some(value.to_string());
            Ok(format!("'{value}'"))
        },
    },
    Setting {
        key: "editor.command",
        apply: |config, value| {
            config.editor.command = Some(value.to_string());
            Ok(format!("'{value}'"))
        },
    },
    Setting {
        key: "display.colors_enabled",
        apply: |config, value| {
            let enabled = parse_bool("display.colors_enabled", value)?;
            config.display.colors_enabled = enabled;
            Ok(enabled.to_string())
        },
    },
    Setting {
        key: "display.date_format",
        apply: |config, value| {
            config.display.date_format = value.to_string();
            Ok(format!("'{value}'"))
        },
    },
    Setting {
        key: "display.stardate_mode",
        apply: |config, value| {
            let enabled = parse_bool("display.stardate_mode", value)?;
            config.display.stardate_mode = enabled;
            Ok(enabled.to_string())
        },
    },
    Setting {
        key: "display.entries_per_page",
        apply: |config, value| {
            if is_auto(value) {
                config.display.entries_per_page = None;
                return Ok(format!("auto (default: {DEFAULT_LIST_LIMIT})"));
            }
            let per_page: usize = value
                .parse()
                .context("display.entries_per_page must be a number or 'auto'")?;
            if per_page == 0 {
                anyhow::bail!("display.entries_per_page must be greater than 0");
            }
            config.display.entries_per_page = Some(per_page);
            Ok(per_page.to_string())
        },
    },
    Setting {
        key: "display.timezone",
        apply: |config, value| {
            if is_auto(value) || value.is_empty() {
                config.display.timezone = None;
                return Ok("auto (system local time)".to_string());
            }
            value.parse::<chrono_tz::Tz>().map_err(|_| {
                anyhow::anyhow!(
                    "Unknown timezone '{value}'. Use an IANA timezone name like 'Europe/Rome'"
                )
            })?;
            config.display.timezone = Some(value.to_string());
            Ok(format!("'{value}'"))
        },
    },
    Setting {
        key: "audio.whisper_command",
        apply: |config, value| {
            config.audio.whisper_command = Some(value.to_string());
            Ok(format!("'{value}'"))
        },
    },
    Setting {
        key: "audio.whisper_model",
        apply: |config, value| {
            config.audio.whisper_model = value.to_string();
            Ok(format!("'{value}'"))
        },
    },
    Setting {
        key: "audio.recording_tool",
        apply: |config, value| {
            config.audio.recording_tool = Some(value.to_string());
            Ok(format!("'{value}'"))
        },
    },
    Setting {
        key: "audio.playback_tool",
        apply: |config, value| {
            config.audio.playback_tool = Some(value.to_string());
            Ok(format!("'{value}'"))
        },
    },
    Setting {
        key: "audio.max_recording_seconds",
        apply: |config, value| {
            let seconds: u64 = value
                .parse()
                .context("audio.max_recording_seconds must be a number")?;
            config.audio.max_recording_seconds = seconds;
            Ok(seconds.to_string())
        },
    },
    Setting {
        key: "audio.sample_rate",
        apply: |config, value| {
            let rate: u32 = value
                .parse()
                .context("audio.sample_rate must be a number")?;
            config.audio.sample_rate = rate;
            Ok(rate.to_string())
        },
    },
];

impl Config {
    /// Set a configuration value by its dotted key (see [`SETTINGS`]).
    /// Returns how the new value should be displayed.
    pub fn set(&mut self, key: &str, value: &str) -> Result<String> {
        let setting = SETTINGS
            .iter()
            .find(|setting| setting.key == key)
            .with_context(|| {
                let keys: Vec<&str> = SETTINGS.iter().map(|setting| setting.key).collect();
                format!(
                    "Unknown configuration key '{key}'. Available keys: {}",
                    keys.join(", ")
                )
            })?;
        (setting.apply)(self, value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn set_applies_known_keys() {
        let mut config = Config::default();
        assert_eq!(
            config.set("editor.command", "code --wait").unwrap(),
            "'code --wait'"
        );
        assert_eq!(config.editor.command.as_deref(), Some("code --wait"));

        config.set("display.stardate_mode", "true").unwrap();
        assert!(config.display.stardate_mode);

        config.set("display.entries_per_page", "auto").unwrap();
        assert_eq!(config.display.entries_per_page, None);

        config.set("display.timezone", "Europe/Rome").unwrap();
        assert_eq!(config.display.timezone.as_deref(), Some("Europe/Rome"));
        config.set("display.timezone", "auto").unwrap();
        assert_eq!(config.display.timezone, None);
    }

    #[test]
    fn set_rejects_bad_values_and_leaves_config_untouched() {
        let mut config = Config::default();
        assert!(config.set("display.colors_enabled", "maybe").is_err());
        assert!(config.set("display.entries_per_page", "0").is_err());
        assert!(config.set("display.timezone", "Mars/Olympus").is_err());
        assert!(config.set("audio.sample_rate", "fast").is_err());
        assert!(config.display.colors_enabled);
        assert_eq!(config.display.entries_per_page, Some(DEFAULT_LIST_LIMIT));
    }

    #[test]
    fn unknown_key_lists_every_valid_key() {
        let err = Config::default().set("nope", "x").unwrap_err().to_string();
        for setting in SETTINGS {
            assert!(
                err.contains(setting.key),
                "{err} is missing {}",
                setting.key
            );
        }
    }
}
