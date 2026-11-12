//! Whisper model files: where they live, which ones exist, downloading and removing them.

use anyhow::{Context, Result, bail};
use directories::ProjectDirs;
use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

const DOWNLOAD_BASE_URL: &str = "https://huggingface.co/ggerganov/whisper.cpp/resolve/main";

/// Offline fallback: the main whisper.cpp models and their download size, checked against
/// the Hugging Face repo. `cl models list --remote` shows the live listing, which also has
/// the quantized variants (`-q5_0`, `-q5_1`, `-q8_0`). Any name is accepted by `install`.
pub const KNOWN_MODELS: &[(&str, &str)] = &[
    ("tiny.en", "74 MB"),
    ("tiny", "74 MB"),
    ("base.en", "141 MB"),
    ("base", "141 MB"),
    ("small.en", "465 MB"),
    ("small", "465 MB"),
    ("medium.en", "1.4 GB"),
    ("medium", "1.4 GB"),
    ("large-v3-turbo", "1.5 GB"),
    ("large-v1", "2.9 GB"),
    ("large-v2", "2.9 GB"),
    ("large-v3", "2.9 GB"),
];

const LISTING_URL: &str = "https://huggingface.co/api/models/ggerganov/whisper.cpp/tree/main";

/// A model available for download.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoteModel {
    pub name: String,
    pub size_bytes: u64,
}

/// Models offered by the whisper.cpp Hugging Face repo, sorted by name.
pub fn fetch_remote() -> Result<Vec<RemoteModel>> {
    let mut response = ureq::get(LISTING_URL)
        .call()
        .with_context(|| format!("Failed to fetch the model list from {LISTING_URL}"))?;
    let body = response
        .body_mut()
        .read_to_string()
        .context("Failed to read the model list")?;
    parse_listing(&body)
}

/// Parse the Hugging Face "tree" JSON into the `ggml-*.bin` models it contains.
fn parse_listing(json: &str) -> Result<Vec<RemoteModel>> {
    let files: Vec<serde_json::Value> =
        serde_json::from_str(json).context("Unexpected model list format")?;
    let mut models: Vec<RemoteModel> = files
        .iter()
        .filter_map(|file| {
            let path = file.get("path")?.as_str()?;
            let name = path.strip_prefix("ggml-")?.strip_suffix(".bin")?;
            // LFS files report their real size under "lfs"
            let size = file
                .pointer("/lfs/size")
                .or_else(|| file.get("size"))?
                .as_u64()?;
            Some(RemoteModel {
                name: name.to_string(),
                size_bytes: size,
            })
        })
        .collect();
    models.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(models)
}

/// A model file found in the models directory.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstalledModel {
    pub name: String,
    pub size_bytes: u64,
}

/// Directory holding the model files (not created here).
pub fn models_dir() -> Result<PathBuf> {
    let proj_dirs =
        ProjectDirs::from("", "", "captains-log").context("Failed to get project directories")?;
    Ok(proj_dirs.data_dir().join("models"))
}

/// Canonical model name: without the `ggml-` prefix and `.bin` suffix.
/// Rejects anything that could escape the models directory or break the URL.
pub fn normalize_name(name: &str) -> Result<String> {
    let name = name.trim();
    let name = name.trim_prefix("ggml-").trim_suffix(".bin");
    let valid = !name.is_empty()
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_'))
        && !name.starts_with('.');
    if !valid {
        bail!("Invalid model name '{name}': use letters, digits, '.', '-' and '_' (e.g. base.en)");
    }
    Ok(name.to_string())
}

pub fn model_filename(name: &str) -> String {
    format!("ggml-{name}.bin")
}

/// Full path of a model file (not created, may not exist).
pub fn model_path(name: &str) -> Result<PathBuf> {
    let name = normalize_name(name)?;
    Ok(models_dir()?.join(model_filename(&name)))
}

/// Models present in `dir`, sorted by name. A missing directory means none.
pub fn installed_in(dir: &Path) -> Result<Vec<InstalledModel>> {
    let entries = match fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(e).with_context(|| format!("Failed to read {}", dir.display())),
    };
    let mut models = Vec::new();
    for entry in entries {
        let entry = entry?;
        let file_name = entry.file_name().to_string_lossy().to_string();
        let Some(name) = file_name
            .strip_prefix("ggml-")
            .and_then(|rest| rest.strip_suffix(".bin"))
        else {
            continue;
        };
        let metadata = entry.metadata()?;
        if metadata.is_file() {
            models.push(InstalledModel {
                name: name.to_string(),
                size_bytes: metadata.len(),
            });
        }
    }
    models.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(models)
}

pub fn installed() -> Result<Vec<InstalledModel>> {
    installed_in(&models_dir()?)
}

pub fn format_size(bytes: u64) -> String {
    const MB: f64 = 1024.0 * 1024.0;
    let b = bytes as f64;
    if b >= 1024.0 * MB {
        format!("{:.1} GB", b / (1024.0 * MB))
    } else if b >= MB {
        format!("{:.0} MB", b / MB)
    } else {
        format!("{:.0} KB", b / 1024.0)
    }
}

/// Download a model into the models directory. Returns the final path.
/// The file is written as `.part` and renamed once complete, so an interrupted
/// download never leaves something that looks like a valid model.
pub fn install(name: &str, force: bool) -> Result<PathBuf> {
    let name = normalize_name(name)?;
    let dir = models_dir()?;
    fs::create_dir_all(&dir)
        .with_context(|| format!("Failed to create models directory at {}", dir.display()))?;

    let filename = model_filename(&name);
    let target = dir.join(&filename);
    if target.exists() && !force {
        bail!(
            "Model '{name}' is already installed at {} (use --force to download it again)",
            target.display()
        );
    }

    let url = format!("{DOWNLOAD_BASE_URL}/{filename}");
    let partial = dir.join(format!("{filename}.part"));
    let result = download(&url, &partial);
    match result {
        Ok(()) => {
            fs::rename(&partial, &target)
                .with_context(|| format!("Failed to move download to {}", target.display()))?;
            Ok(target)
        }
        Err(e) => {
            let _ = fs::remove_file(&partial);
            Err(e)
        }
    }
}

fn download(url: &str, dest: &Path) -> Result<()> {
    let response = match ureq::get(url).call() {
        Ok(response) => response,
        Err(ureq::Error::StatusCode(404)) => {
            bail!("No such model at {url} (404). Run `cl models list` for the known models")
        }
        Err(e) => return Err(e).with_context(|| format!("Failed to download {url}")),
    };
    let total: Option<u64> = response
        .headers()
        .get("content-length")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.parse().ok());

    let mut reader = response.into_body().into_reader();
    let mut file =
        fs::File::create(dest).with_context(|| format!("Failed to create {}", dest.display()))?;
    let mut buf = vec![0u8; 64 * 1024];
    let mut done: u64 = 0;
    let mut last_percent = u64::MAX;
    loop {
        let n = reader.read(&mut buf).context("Download interrupted")?;
        if n == 0 {
            break;
        }
        file.write_all(&buf[..n])?;
        done += n as u64;
        match total {
            Some(total) if total > 0 => {
                let percent = done * 100 / total;
                if percent != last_percent {
                    last_percent = percent;
                    eprint!(
                        "\r  {percent:>3}%  {} / {}",
                        format_size(done),
                        format_size(total)
                    );
                }
            }
            _ => eprint!("\r  {}", format_size(done)),
        }
    }
    eprintln!();
    file.flush()?;

    if let Some(total) = total
        && done != total
    {
        bail!("Download incomplete: got {done} of {total} bytes");
    }
    Ok(())
}

/// Delete an installed model. Returns false if it wasn't installed.
pub fn remove(name: &str) -> Result<bool> {
    let path = model_path(name)?;
    match fs::remove_file(&path) {
        Ok(()) => Ok(true),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(e) => Err(e).with_context(|| format!("Failed to delete {}", path.display())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_strips_prefix_and_suffix() {
        assert_eq!(normalize_name("base.en").unwrap(), "base.en");
        assert_eq!(normalize_name("ggml-base.en.bin").unwrap(), "base.en");
        assert_eq!(normalize_name(" large-v3-q5_0 ").unwrap(), "large-v3-q5_0");
    }

    #[test]
    fn normalize_rejects_paths_and_junk() {
        for bad in [
            "",
            "..",
            "../x",
            "a/b",
            "a\\b",
            ".hidden",
            "a b",
            "a?b=c",
            "ggml-.bin",
        ] {
            assert!(normalize_name(bad).is_err(), "{bad:?} should be rejected");
        }
    }

    #[test]
    fn installed_in_lists_only_model_files() {
        let dir = std::env::temp_dir().join(format!("cl-models-test-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        assert!(installed_in(&dir).unwrap().is_empty());

        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("ggml-small.en.bin"), vec![0u8; 10]).unwrap();
        fs::write(dir.join("ggml-base.en.bin"), vec![0u8; 5]).unwrap();
        fs::write(dir.join("ggml-base.en.bin.part"), b"x").unwrap();
        fs::write(dir.join("notes.txt"), b"x").unwrap();

        let models = installed_in(&dir).unwrap();
        let _ = fs::remove_dir_all(&dir);
        assert_eq!(
            models,
            vec![
                InstalledModel {
                    name: "base.en".into(),
                    size_bytes: 5
                },
                InstalledModel {
                    name: "small.en".into(),
                    size_bytes: 10
                },
            ]
        );
    }

    #[test]
    fn parse_listing_keeps_model_files_with_lfs_sizes() {
        let json = r#"[
            {"type":"file","size":134,"path":"ggml-tiny.bin","lfs":{"size":77691713}},
            {"type":"file","size":50,"path":"ggml-base.en-q5_1.bin","lfs":{"size":59721011}},
            {"type":"file","size":900,"path":"README.md"},
            {"type":"file","size":10,"path":"ggml-small.bin.part"},
            {"type":"directory","path":"models"}
        ]"#;
        assert_eq!(
            parse_listing(json).unwrap(),
            vec![
                RemoteModel {
                    name: "base.en-q5_1".into(),
                    size_bytes: 59721011
                },
                RemoteModel {
                    name: "tiny".into(),
                    size_bytes: 77691713
                },
            ]
        );
        assert!(parse_listing("{}").is_err());
    }

    #[test]
    fn sizes_are_readable() {
        assert_eq!(format_size(2048), "2 KB");
        assert_eq!(format_size(150 * 1024 * 1024), "150 MB");
        assert_eq!(format_size(3 * 1024 * 1024 * 1024), "3.0 GB");
    }
}
