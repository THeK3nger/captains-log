use crate::journal::{Entry, Journal};
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

#[derive(Debug, Serialize, Deserialize)]
pub struct ExportData {
    pub version: String,
    pub exported_at: chrono::DateTime<chrono::Utc>,
    pub entries: Vec<Entry>,
}

pub struct Exporter<'a> {
    journal: &'a Journal,
}

impl<'a> Exporter<'a> {
    pub fn new(journal: &'a Journal) -> Self {
        Self { journal }
    }

    pub fn export_to_json(
        &self,
        output_path: Option<String>,
        filters: Option<ExportFilters>,
    ) -> Result<()> {
        let entries = if let Some(filters) = filters {
            self.journal.list_entries_filtered(
                filters.date.as_deref(),
                filters.since.as_deref(),
                filters.until.as_deref(),
                filters.journal.as_deref(),
            )?
        } else {
            self.journal.list_entries()?
        };

        let export_data = ExportData {
            version: env!("CARGO_PKG_VERSION").to_string(),
            exported_at: chrono::Utc::now(),
            entries,
        };

        let json_content = serde_json::to_string_pretty(&export_data)
            .context("Failed to serialize entries to JSON")?;

        if output_path.is_some() {
            let path: &str = output_path.as_deref().unwrap();
            // Create directory if it doesn't exist
            if let Some(parent) = Path::new(path).parent() {
                fs::create_dir_all(parent).context("Failed to create output directory")?;
            }

            fs::write(path, json_content).context("Failed to write JSON file")?;
        } else {
            println!("{}", json_content);
        }

        Ok(())
    }
}

#[derive(Debug)]
pub struct ExportFilters {
    pub date: Option<String>,
    pub since: Option<String>,
    pub until: Option<String>,
    pub journal: Option<String>,
}
