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
const BLOCK_ALIGN: u16 = CHANNELS * BITS_PER_SAMPLE / 8;
const WAV_HEADER_LEN: u32 = 44;
// RIFF's size excludes its first eight bytes and must fit in a u32.
const MAX_PCM_BYTES: u32 =
    (u32::MAX - (WAV_HEADER_LEN - 8)) / BLOCK_ALIGN as u32 * BLOCK_ALIGN as u32;

/// Record audio to the specified output path (16-bit mono WAV).
///
/// The recording tool only supplies raw PCM on its stdout; this process owns the
/// WAV file. Stopping is therefore just "set a flag, kill the tool, fix the
/// header": no signals. Stream errors still finalize the complete samples
/// captured so far; storage errors are returned if finalization fails.
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

    let writer = spawn_pcm_writer(stdout, file, sample_rate, Arc::clone(&stop));
    let outcome = wait_for_stop(&mut child, &stop, &writer, max_duration);

    // Killing the tool closes its stdout, which ends the writer thread.
    let _ = child.kill();
    let _ = child.wait();
    let pcm_bytes = writer
        .join()
        .map_err(|_| anyhow::anyhow!("Recording writer thread panicked"))??;
    outcome?;

    if pcm_bytes == 0 {
        return Err(anyhow::anyhow!(
            "No audio was captured (is the microphone available?)"
        ));
    }

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

/// Copy and finalize PCM, including on stream errors. Wake the recording loop
/// on failure so a full WAV or a failed write stops the tool promptly.
fn spawn_pcm_writer(
    mut source: impl Read + Send + 'static,
    mut file: File,
    sample_rate: u32,
    stop: Arc<AtomicBool>,
) -> JoinHandle<Result<u32>> {
    thread::spawn(move || {
        let result = write_pcm(&mut source, &mut file, sample_rate, MAX_PCM_BYTES);
        if result.is_err() {
            stop.store(true, Ordering::Relaxed);
        }
        result
    })
}

fn write_pcm(
    mut source: impl Read,
    file: &mut File,
    sample_rate: u32,
    max_pcm_bytes: u32,
) -> Result<u32> {
    let outcome = (|| -> Result<()> {
        file.seek(SeekFrom::Start(WAV_HEADER_LEN as u64))?;
        let mut buf = [0u8; 8192];
        let mut total = 0u32;
        loop {
            let n = source.read(&mut buf)?;
            if n == 0 {
                return Ok(());
            }
            let count = n.min((max_pcm_bytes - total) as usize);
            file.write_all(&buf[..count])?;
            total += count as u32;
            anyhow::ensure!(count == n, "Recording exceeds the PCM WAV size limit");
        }
    })();

    // Use the actual length: write_all may have written only part of a buffer
    // before failing. Discard any incomplete sample, but not odd read chunks.
    let data_len = file.metadata()?.len().saturating_sub(WAV_HEADER_LEN as u64);
    let data_len = u32::try_from(data_len).context("Recording exceeds the PCM WAV size limit")?;
    let pcm_bytes = data_len - data_len % BLOCK_ALIGN as u32;
    file.set_len(WAV_HEADER_LEN as u64 + pcm_bytes as u64)?;
    write_wav_header(file, sample_rate, pcm_bytes)?;
    outcome?;
    Ok(pcm_bytes)
}

/// Canonical 44-byte PCM WAV header. `data_len` is patched in once known.
fn write_wav_header(file: &mut File, sample_rate: u32, data_len: u32) -> Result<()> {
    anyhow::ensure!(
        data_len <= MAX_PCM_BYTES && data_len.is_multiple_of(BLOCK_ALIGN as u32),
        "PCM WAV data must contain complete samples and fit the RIFF size limit"
    );
    let mut h = [0u8; WAV_HEADER_LEN as usize];
    h[0..4].copy_from_slice(b"RIFF");
    h[4..8].copy_from_slice(&(WAV_HEADER_LEN - 8 + data_len).to_le_bytes());
    h[8..16].copy_from_slice(b"WAVEfmt ");
    h[16..20].copy_from_slice(&16u32.to_le_bytes());
    h[20..22].copy_from_slice(&1u16.to_le_bytes()); // PCM
    h[22..24].copy_from_slice(&CHANNELS.to_le_bytes());
    h[24..28].copy_from_slice(&sample_rate.to_le_bytes());
    h[28..32].copy_from_slice(&(sample_rate * BLOCK_ALIGN as u32).to_le_bytes());
    h[32..34].copy_from_slice(&BLOCK_ALIGN.to_le_bytes());
    h[34..36].copy_from_slice(&BITS_PER_SAMPLE.to_le_bytes());
    h[36..40].copy_from_slice(b"data");
    h[40..44].copy_from_slice(&data_len.to_le_bytes());

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
    writer: &JoinHandle<Result<u32>>,
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

#[cfg(test)]
mod tests {
    use super::*;

    fn with_wav<T>(run: impl FnOnce(&mut File) -> T) -> T {
        let path = std::env::temp_dir().join(format!(
            "captains-log-wav-{}-{}.wav",
            std::process::id(),
            rand::random::<u64>()
        ));
        let mut file = File::options()
            .read(true)
            .write(true)
            .create_new(true)
            .open(&path)
            .unwrap();
        write_wav_header(&mut file, 16000, 0).unwrap();
        let result = run(&mut file);
        drop(file);
        std::fs::remove_file(path).unwrap();
        result
    }

    fn capture(source: impl Read, limit: u32) -> (Result<u32>, Vec<u8>) {
        with_wav(|file| {
            let result = write_pcm(source, file, 16000, limit);
            file.rewind().unwrap();
            let mut bytes = Vec::new();
            file.read_to_end(&mut bytes).unwrap();
            (result, bytes)
        })
    }

    fn assert_pcm(bytes: &[u8], expected: &[u8]) {
        let riff_len = u32::from_le_bytes(bytes[4..8].try_into().unwrap());
        let data_len = u32::from_le_bytes(bytes[40..44].try_into().unwrap());
        assert_eq!(riff_len as usize + 8, bytes.len());
        assert_eq!(data_len as usize, expected.len());
        assert_eq!(&bytes[WAV_HEADER_LEN as usize..], expected);
        assert!(data_len.is_multiple_of(BLOCK_ALIGN as u32));
    }

    struct ChunkedPcm {
        bytes: &'static [u8],
        fail_at_end: bool,
    }

    impl Read for ChunkedPcm {
        fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
            if self.bytes.is_empty() && self.fail_at_end {
                return Err(std::io::Error::other("PCM source failed"));
            }
            let n = buf.len().min(self.bytes.len()).min(3);
            buf[..n].copy_from_slice(&self.bytes[..n]);
            self.bytes = &self.bytes[n..];
            Ok(n)
        }
    }

    #[test]
    fn preserves_samples_across_odd_reads_and_trims_incomplete_tail() {
        let (result, bytes) = capture(
            ChunkedPcm {
                bytes: &[1, 2, 3, 4, 5],
                fail_at_end: false,
            },
            MAX_PCM_BYTES,
        );
        assert_eq!(result.unwrap(), 4);
        assert_pcm(&bytes, &[1, 2, 3, 4]);
    }

    #[test]
    fn finalizes_complete_samples_before_returning_source_error() {
        let (result, bytes) = capture(
            ChunkedPcm {
                bytes: &[1, 2, 3, 4, 5],
                fail_at_end: true,
            },
            MAX_PCM_BYTES,
        );
        assert!(result.is_err());
        assert_pcm(&bytes, &[1, 2, 3, 4]);
    }

    #[test]
    fn size_limit_keeps_valid_prefix_and_returns_error() {
        let (result, bytes) = capture(&[1, 2, 3, 4, 5, 6][..], 4);
        assert!(result.is_err());
        assert_pcm(&bytes, &[1, 2, 3, 4]);
    }

    #[test]
    fn exact_size_limit_is_accepted() {
        let (result, bytes) = capture(&[1, 2, 3, 4][..], 4);
        assert_eq!(result.unwrap(), 4);
        assert_pcm(&bytes, &[1, 2, 3, 4]);
    }

    #[test]
    fn header_checks_riff_size_and_sample_boundaries() {
        let (header, odd, oversized) = with_wav(|file| {
            write_wav_header(file, 16000, MAX_PCM_BYTES).unwrap();
            file.rewind().unwrap();
            let mut header = [0u8; WAV_HEADER_LEN as usize];
            file.read_exact(&mut header).unwrap();
            (
                header,
                write_wav_header(file, 16000, MAX_PCM_BYTES + 1),
                write_wav_header(file, 16000, MAX_PCM_BYTES + 2),
            )
        });
        assert_eq!(
            u32::from_le_bytes(header[4..8].try_into().unwrap()),
            MAX_PCM_BYTES + (WAV_HEADER_LEN - 8)
        );
        assert_eq!(
            u32::from_le_bytes(header[40..44].try_into().unwrap()),
            MAX_PCM_BYTES
        );
        assert!(odd.is_err());
        assert!(oversized.is_err());
    }
}
