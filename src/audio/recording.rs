use crate::audio::platform::{Platform, detect_platform, detect_recording_tool};
use crate::config::Config;
use anyhow::{Context, Result};
use colored::Colorize;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

const CHANNELS: u16 = 1;
const BITS_PER_SAMPLE: u16 = 16;
const WAV_HEADER_LEN: u32 = 44;

/// Record audio to the specified output path (16-bit mono WAV).
///
/// The recording tool only supplies raw PCM on its stdout; this process owns the
/// WAV file. Stopping is therefore just "set a flag, kill the tool, fix the
/// header": no signals, and the file is valid however the tool dies.
/// Blocks until Enter or Ctrl+C is pressed, or max_duration is reached.
pub fn record_audio(config: &Config, output_path: &Path, max_duration: u64) -> Result<Duration> {
    let tool = config
        .audio
        .recording_tool
        .clone()
        .or_else(|| detect_recording_tool().ok())
        .context("No recording tool available")?;

    let sample_rate = config.audio.sample_rate;
    let mut cmd = build_recording_command(&tool, sample_rate)?;

    let mut file = File::create(output_path).context("Failed to create recording file")?;
    write_wav_header(&mut file, sample_rate, 0)?;

    let mut child = cmd.spawn().context("Failed to start recording process")?;
    let stdout = child
        .stdout
        .take()
        .context("Failed to capture recording output")?;

    println!(
        "{}",
        "🎤 Recording started. Press Enter or Ctrl+C to stop...".cyan()
    );
    println!(
        "{}",
        format!("⏺️  Recording... (max {} seconds)", max_duration).yellow()
    );

    let stop = Arc::new(AtomicBool::new(false));
    install_stop_triggers(&stop);

    let writer = spawn_pcm_writer(stdout, file);
    let outcome = wait_for_stop(&mut child, &stop, &writer, max_duration);

    // Killing the tool closes its stdout, which ends the writer thread.
    let _ = child.kill();
    let _ = child.wait();
    let (mut file, pcm_bytes) = writer
        .join()
        .map_err(|_| anyhow::anyhow!("Recording writer thread panicked"))??;
    outcome?;

    if pcm_bytes == 0 {
        return Err(anyhow::anyhow!(
            "No audio was captured (is the microphone available?)"
        ));
    }
    write_wav_header(&mut file, sample_rate, pcm_bytes)?;

    let bytes_per_second = sample_rate as f64 * (CHANNELS * BITS_PER_SAMPLE / 8) as f64;
    let duration = Duration::from_secs_f64(pcm_bytes as f64 / bytes_per_second);
    println!(
        "{}",
        format!(
            "✓ Recording stopped ({:.1} seconds)",
            duration.as_secs_f64()
        )
        .green()
    );
    Ok(duration)
}

/// Ctrl+C and Enter both just raise the stop flag.
fn install_stop_triggers(stop: &Arc<AtomicBool>) {
    let on_ctrlc = stop.clone();
    // Fails only if a handler is already installed; Enter still works then.
    let _ = ctrlc::set_handler(move || on_ctrlc.store(true, Ordering::Relaxed));

    let on_enter = stop.clone();
    // Detached on purpose: it may stay blocked on stdin until the process exits.
    thread::spawn(move || {
        let mut line = String::new();
        // Ok(0) is EOF (stdin closed or redirected): not a request to stop.
        if matches!(std::io::stdin().read_line(&mut line), Ok(n) if n > 0) {
            on_enter.store(true, Ordering::Relaxed);
        }
    });
}

/// Copy the tool's raw PCM into the WAV file, after the header. Returns the file
/// and the number of PCM bytes written.
fn spawn_pcm_writer(
    mut source: impl Read + Send + 'static,
    mut file: File,
) -> JoinHandle<Result<(File, u32)>> {
    thread::spawn(move || {
        file.seek(SeekFrom::Start(WAV_HEADER_LEN as u64))?;
        let mut buf = [0u8; 8192];
        let mut total: u32 = 0;
        loop {
            let n = source.read(&mut buf)?;
            if n == 0 {
                break;
            }
            file.write_all(&buf[..n])?;
            total = total.saturating_add(n as u32);
        }
        file.flush()?;
        Ok((file, total))
    })
}

/// Canonical 44-byte PCM WAV header. `data_len` is patched in once known.
fn write_wav_header(file: &mut File, sample_rate: u32, data_len: u32) -> Result<()> {
    let block_align = CHANNELS * BITS_PER_SAMPLE / 8;
    let mut h = Vec::with_capacity(WAV_HEADER_LEN as usize);
    h.extend_from_slice(b"RIFF");
    h.extend_from_slice(&(36 + data_len).to_le_bytes());
    h.extend_from_slice(b"WAVEfmt ");
    h.extend_from_slice(&16u32.to_le_bytes());
    h.extend_from_slice(&1u16.to_le_bytes()); // PCM
    h.extend_from_slice(&CHANNELS.to_le_bytes());
    h.extend_from_slice(&sample_rate.to_le_bytes());
    h.extend_from_slice(&(sample_rate * block_align as u32).to_le_bytes());
    h.extend_from_slice(&block_align.to_le_bytes());
    h.extend_from_slice(&BITS_PER_SAMPLE.to_le_bytes());
    h.extend_from_slice(b"data");
    h.extend_from_slice(&data_len.to_le_bytes());

    file.seek(SeekFrom::Start(0))?;
    file.write_all(&h)?;
    file.flush()?;
    Ok(())
}

/// Build the recording command: raw 16-bit little-endian mono PCM on stdout.
fn build_recording_command(tool: &str, sample_rate: u32) -> Result<Command> {
    let platform = detect_platform();
    let sample_rate = sample_rate.to_string();

    let mut cmd = Command::new(tool);

    match (tool, platform) {
        ("sox", Platform::MacOS) | ("sox", Platform::Linux) => {
            cmd.args([
                "-q",
                "-d", // Default device
                "-r",
                &sample_rate,
                "-c",
                "1", // Mono
                "-b",
                "16", // 16-bit
                "-e",
                "signed-integer",
                "-L", // Little-endian
                "-t",
                "raw",
                "-", // stdout
            ]);
        }
        ("arecord", Platform::Linux) => {
            cmd.args([
                "-q",
                "-t",
                "raw",
                "-f",
                "S16_LE", // 16-bit little-endian
                "-c",
                "1", // Mono
                "-r",
                &sample_rate,
                "-", // stdout
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

            cmd.args([
                "-nostdin", // keep Enter for us
                "-loglevel",
                "error",
                "-f",
                input_device,
                "-i",
                input_arg,
                "-ar",
                &sample_rate,
                "-ac",
                "1",
                "-f",
                "s16le",
                "pipe:1",
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

    // stdin stays ours (Enter stops the recording); stdout carries the audio.
    cmd.stdin(Stdio::null());
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::null());

    Ok(cmd)
}

/// Wait until the stop flag is raised or max duration is reached. Errors if the
/// tool dies on its own before then.
fn wait_for_stop(
    child: &mut Child,
    stop: &AtomicBool,
    writer: &JoinHandle<Result<(File, u32)>>,
    max_duration: u64,
) -> Result<()> {
    let start = Instant::now();
    let max = Duration::from_secs(max_duration);

    loop {
        if stop.load(Ordering::Relaxed) {
            return Ok(());
        }

        if start.elapsed() >= max {
            println!("{}", "⏱️  Maximum duration reached, stopping...".yellow());
            return Ok(());
        }

        // Tool exited by itself (bad device, missing permission, ...)
        if let Some(status) = child
            .try_wait()
            .context("Failed to check recording process")?
        {
            // Ctrl+C reaches the tool too (same process group) and may beat our own
            // handler to it; give the flag a moment before calling this a failure.
            thread::sleep(Duration::from_millis(200));
            if stop.load(Ordering::Relaxed) || writer.is_finished() && status.success() {
                return Ok(());
            }
            return Err(anyhow::anyhow!(
                "Recording process exited unexpectedly with status: {}",
                status
            ));
        }

        thread::sleep(Duration::from_millis(50));
    }
}
