use crate::audio::platform::{Platform, detect_platform, detect_recording_tool};
use crate::config::Config;
use anyhow::{Context, Result};
use colored::Colorize;
use signal_hook::consts::SIGINT;
use signal_hook::iterator::Signals;
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::{Duration, Instant};

/// Record audio to the specified output path
/// Blocks until recording is stopped (Ctrl+C) or max_duration is reached
pub fn record_audio(config: &Config, output_path: &Path, max_duration: u64) -> Result<Duration> {
    // Get recording tool
    let tool = config
        .audio
        .recording_tool
        .clone()
        .or_else(|| detect_recording_tool().ok())
        .context("No recording tool available")?;

    // Build platform-specific recording command
    let mut cmd = build_recording_command(&tool, output_path, config)?;

    println!("{}", "🎤 Recording started. Press Ctrl+C to stop...".cyan());
    println!(
        "{}",
        format!("⏺️  Recording... (max {} seconds)", max_duration).yellow()
    );

    // Start recording
    let mut child = cmd.spawn().context("Failed to start recording process")?;

    // Set up Ctrl+C handler
    let interrupted = Arc::new(AtomicBool::new(false));
    let interrupted_clone = interrupted.clone();

    // Spawn signal handler thread
    thread::spawn(move || {
        let mut signals = Signals::new(&[SIGINT]).expect("Failed to register signal handler");
        for _ in signals.forever() {
            interrupted_clone.store(true, Ordering::Relaxed);
            break;
        }
    });

    // Monitor recording duration and interruption
    let start = Instant::now();
    let result = monitor_recording(&mut child, &interrupted, max_duration);

    // Get actual duration
    let duration = start.elapsed();

    // Handle result
    match result {
        Ok(()) => {
            println!(
                "{}",
                format!(
                    "✓ Recording stopped ({:.1} seconds)",
                    duration.as_secs_f64()
                )
                .green()
            );
        }
        Err(e) => {
            // Try to kill the process if it's still running
            let _ = child.kill();
            return Err(e);
        }
    }

    // Verify file exists and has content
    if !output_path.exists() {
        return Err(anyhow::anyhow!("Recording file was not created"));
    }

    let file_size = std::fs::metadata(output_path)
        .context("Failed to check recording file")?
        .len();

    if file_size == 0 {
        return Err(anyhow::anyhow!("Recording file is empty"));
    }

    Ok(duration)
}

/// Build the recording command based on tool and platform
fn build_recording_command(tool: &str, output: &Path, config: &Config) -> Result<Command> {
    let platform = detect_platform();
    let sample_rate = config.audio.sample_rate.to_string();
    let output_str = output.to_str().context("Invalid output path")?;

    let mut cmd = Command::new(tool);

    match (tool, platform) {
        ("sox", Platform::MacOS) | ("sox", Platform::Linux) => {
            cmd.args(&[
                "-d", // Default device
                "-r",
                &sample_rate,
                "-c",
                "1", // Mono
                "-b",
                "16", // 16-bit
                output_str,
            ]);
        }
        ("arecord", Platform::Linux) => {
            cmd.args(&[
                "-f",
                "S16_LE", // 16-bit little-endian
                "-c",
                "1", // Mono
                "-r",
                &sample_rate,
                output_str,
            ]);
        }
        ("ffmpeg", _) => {
            let input_device = match platform {
                Platform::MacOS => "avfoundation",
                Platform::Linux => "pulse",
                Platform::Windows => "dshow",
            };

            let input_arg = match platform {
                Platform::MacOS => ":0", // Default audio input
                Platform::Linux => "default",
                Platform::Windows => "audio=\"Microphone\"",
            };

            cmd.args(&[
                "-f",
                input_device,
                "-i",
                input_arg,
                "-ar",
                &sample_rate,
                "-ac",
                "1",
                "-y", // Overwrite output
                output_str,
            ]);
        }
        _ => {
            return Err(anyhow::anyhow!(
                "Unsupported recording tool: {} for platform: {:?}",
                tool,
                platform
            ));
        }
    }

    // Suppress stdout/stderr to keep terminal clean
    cmd.stdout(Stdio::null());
    cmd.stderr(Stdio::null());

    Ok(cmd)
}

/// Monitor recording process until interrupted or max duration reached
fn monitor_recording(
    child: &mut Child,
    interrupted: &Arc<AtomicBool>,
    max_duration: u64,
) -> Result<()> {
    let start = Instant::now();
    let max = Duration::from_secs(max_duration);

    loop {
        // Check if interrupted
        if interrupted.load(Ordering::Relaxed) {
            // Send SIGINT to child process to stop gracefully
            #[cfg(unix)]
            {
                let pid = child.id() as i32;
                unsafe {
                    libc::kill(pid, libc::SIGINT);
                }
            }

            // Wait a moment for graceful shutdown
            thread::sleep(Duration::from_millis(500));

            // Force kill if still running
            let _ = child.kill();
            let _ = child.wait();

            return Ok(());
        }

        // Check if max duration reached
        if start.elapsed() >= max {
            println!("{}", "⏱️  Maximum duration reached, stopping...".yellow());

            // Send SIGINT to child process
            #[cfg(unix)]
            {
                let pid = child.id() as i32;
                unsafe {
                    libc::kill(pid, libc::SIGINT);
                }
            }

            thread::sleep(Duration::from_millis(500));
            let _ = child.kill();
            let _ = child.wait();

            return Ok(());
        }

        // Check if process exited on its own (error)
        match child.try_wait() {
            Ok(Some(status)) => {
                if !status.success() {
                    return Err(anyhow::anyhow!(
                        "Recording process exited unexpectedly with status: {}",
                        status
                    ));
                }
                // Process exited successfully (unlikely for recording)
                return Ok(());
            }
            Ok(None) => {
                // Still running, continue monitoring
            }
            Err(e) => {
                return Err(anyhow::anyhow!("Failed to check recording process: {}", e));
            }
        }

        // Sleep briefly to avoid busy waiting
        thread::sleep(Duration::from_millis(100));
    }
}
