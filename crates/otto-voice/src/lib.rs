//! Local voice I/O: microphone capture + speech-to-text (whisper.cpp, via
//! `whisper-rs`) for prompts, and text-to-speech (the OS's native speech
//! engine, via the `tts` crate) for talk-back. Both run fully offline - no
//! audio ever leaves the machine.
//!
//! `cpal::Stream` and `tts::Tts` are both `!Send` (they wrap platform
//! handles / an `Rc`), so they can't live in async task state directly.
//! Instead, a single dedicated OS thread owns both for the lifetime of the
//! app; [`VoiceEngine`] is just a cheap, `Send + Sync` channel handle to it.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use anyhow::{anyhow, bail, Context, Result};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use tts::Tts;
use whisper_rs::{FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters};

/// Where the whisper.cpp GGML model lives by default (`OTTO_WHISPER_MODEL`
/// overrides this). Not bundled with the crate - see the README for how to
/// download one.
pub fn default_model_path() -> Result<PathBuf> {
    if let Ok(path) = std::env::var("OTTO_WHISPER_MODEL") {
        return Ok(PathBuf::from(path));
    }
    let dirs = directories::ProjectDirs::from("dev", "ayimdomnic", "otto")
        .ok_or_else(|| anyhow!("could not resolve a data directory for this platform"))?;
    Ok(dirs.data_dir().join("models").join("ggml-base.en.bin"))
}

enum Command {
    StartRecording,
    StopRecording(tokio::sync::oneshot::Sender<Result<String>>),
    Speak(String, tokio::sync::oneshot::Sender<Result<()>>),
}

/// A cheap, cloneable, `Send + Sync` handle to the voice worker thread.
#[derive(Clone)]
pub struct VoiceEngine {
    tx: std::sync::mpsc::Sender<Command>,
}

impl VoiceEngine {
    /// Spawns the voice worker thread, loading the whisper model from
    /// `model_path`. Fails fast if the model file doesn't exist, rather
    /// than spawning a thread that can only ever fail later.
    pub fn spawn(model_path: impl Into<PathBuf>) -> Result<Self> {
        let model_path = model_path.into();
        if !model_path.exists() {
            bail!(
                "whisper model not found at {} - see the README for how to download one, or set OTTO_WHISPER_MODEL",
                model_path.display()
            );
        }

        let (tx, rx) = std::sync::mpsc::channel::<Command>();
        std::thread::Builder::new()
            .name("otto-voice".into())
            .spawn(move || worker_loop(model_path, rx))
            .context("failed to spawn voice worker thread")?;

        Ok(Self { tx })
    }

    /// Starts capturing from the default input device. A no-op if already
    /// recording.
    pub async fn start_recording(&self) -> Result<()> {
        self.tx
            .send(Command::StartRecording)
            .map_err(|_| anyhow!("voice worker thread is gone"))
    }

    /// Stops capturing and transcribes what was captured. Errors if nothing
    /// was recording.
    pub async fn stop_recording(&self) -> Result<String> {
        let (reply_tx, reply_rx) = tokio::sync::oneshot::channel();
        self.tx
            .send(Command::StopRecording(reply_tx))
            .map_err(|_| anyhow!("voice worker thread is gone"))?;
        reply_rx
            .await
            .context("voice worker thread dropped the reply channel")?
    }

    /// Speaks `text` aloud, interrupting anything currently being spoken.
    pub async fn speak(&self, text: impl Into<String>) -> Result<()> {
        let (reply_tx, reply_rx) = tokio::sync::oneshot::channel();
        self.tx
            .send(Command::Speak(text.into(), reply_tx))
            .map_err(|_| anyhow!("voice worker thread is gone"))?;
        reply_rx
            .await
            .context("voice worker thread dropped the reply channel")?
    }
}

struct Recording {
    stream: cpal::Stream,
    buffer: Arc<Mutex<Vec<f32>>>,
    sample_rate: u32,
    channels: u16,
}

fn worker_loop(model_path: PathBuf, rx: std::sync::mpsc::Receiver<Command>) {
    let ctx =
        match WhisperContext::new_with_params(&model_path, WhisperContextParameters::default()) {
            Ok(ctx) => ctx,
            Err(e) => {
                tracing::error!(
                    "failed to load whisper model from {}: {e}",
                    model_path.display()
                );
                return;
            }
        };

    let mut speaker = match Tts::default() {
        Ok(tts) => Some(tts),
        Err(e) => {
            tracing::warn!(
                "no text-to-speech backend available ({e}) - speak() will report an error"
            );
            None
        }
    };

    let host = cpal::default_host();
    let mut recording: Option<Recording> = None;

    while let Ok(cmd) = rx.recv() {
        match cmd {
            Command::StartRecording => {
                if recording.is_some() {
                    tracing::debug!("start_recording called while already recording; ignoring");
                    continue;
                }
                match start_capture(&host) {
                    Ok(r) => recording = Some(r),
                    Err(e) => tracing::error!("failed to start recording: {e}"),
                }
            }
            Command::StopRecording(reply) => {
                let result = stop_and_transcribe(&ctx, recording.take());
                let _ = reply.send(result);
            }
            Command::Speak(text, reply) => {
                let result = match speaker.as_mut() {
                    Some(speaker) => speaker
                        .speak(text, true)
                        .map(|_| ())
                        .map_err(|e| anyhow!(e.to_string())),
                    None => Err(anyhow!(
                        "no text-to-speech backend available on this system"
                    )),
                };
                let _ = reply.send(result);
            }
        }
    }
}

fn start_capture(host: &cpal::Host) -> Result<Recording> {
    let device = host
        .default_input_device()
        .ok_or_else(|| anyhow!("no default input (microphone) device available"))?;
    let supported = device
        .default_input_config()
        .context("failed to read default input config")?;
    let sample_rate = supported.sample_rate();
    let channels = supported.channels();
    let sample_format = supported.sample_format();
    let config = supported.config();

    let buffer = Arc::new(Mutex::new(Vec::<f32>::new()));
    let buffer_cb = buffer.clone();

    let error_callback = |err: cpal::Error| tracing::error!("audio input stream error: {err}");

    let stream = match sample_format {
        cpal::SampleFormat::F32 => device.build_input_stream(
            config,
            move |data: &[f32], _: &cpal::InputCallbackInfo| {
                buffer_cb
                    .lock()
                    .expect("audio buffer mutex poisoned")
                    .extend_from_slice(data);
            },
            error_callback,
            None,
        ),
        cpal::SampleFormat::I16 => device.build_input_stream(
            config,
            move |data: &[i16], _: &cpal::InputCallbackInfo| {
                let mut buf = buffer_cb.lock().expect("audio buffer mutex poisoned");
                buf.extend(data.iter().map(|s| *s as f32 / i16::MAX as f32));
            },
            error_callback,
            None,
        ),
        other => bail!("unsupported input sample format: {other:?}"),
    }
    .context("failed to build input stream")?;

    stream.play().context("failed to start input stream")?;

    Ok(Recording {
        stream,
        buffer,
        sample_rate,
        channels,
    })
}

fn stop_and_transcribe(ctx: &WhisperContext, recording: Option<Recording>) -> Result<String> {
    let recording = recording.ok_or_else(|| anyhow!("not currently recording"))?;
    // Dropping the stream stops capture (cpal streams stop on drop).
    drop(recording.stream);

    let samples = std::mem::take(
        &mut *recording
            .buffer
            .lock()
            .expect("audio buffer mutex poisoned"),
    );
    if samples.is_empty() {
        bail!("no audio was captured");
    }

    let mono = to_mono(&samples, recording.channels);
    let pcm_16k = resample_linear(&mono, recording.sample_rate, 16_000);
    transcribe(ctx, &pcm_16k)
}

fn to_mono(samples: &[f32], channels: u16) -> Vec<f32> {
    if channels <= 1 {
        return samples.to_vec();
    }
    let channels = channels as usize;
    samples
        .chunks_exact(channels)
        .map(|frame| frame.iter().sum::<f32>() / channels as f32)
        .collect()
}

/// Linear-interpolation resampler. Not audiophile quality, but whisper.cpp
/// only needs 16kHz mono speech - good enough for that, and avoids pulling
/// in a full resampling crate for one job.
fn resample_linear(samples: &[f32], from_rate: u32, to_rate: u32) -> Vec<f32> {
    if from_rate == to_rate || samples.is_empty() {
        return samples.to_vec();
    }
    let ratio = from_rate as f64 / to_rate as f64;
    let out_len = (samples.len() as f64 / ratio).ceil() as usize;
    (0..out_len)
        .map(|i| {
            let src_pos = i as f64 * ratio;
            let idx = src_pos.floor() as usize;
            let frac = (src_pos - idx as f64) as f32;
            let a = samples[idx.min(samples.len() - 1)];
            let b = samples[(idx + 1).min(samples.len() - 1)];
            a + (b - a) * frac
        })
        .collect()
}

fn transcribe(ctx: &WhisperContext, pcm_16k_mono: &[f32]) -> Result<String> {
    let mut state = ctx
        .create_state()
        .context("failed to create whisper state")?;

    let mut params = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });
    params.set_print_progress(false);
    params.set_print_special(false);
    params.set_print_realtime(false);
    params.set_language(Some("en"));

    state
        .full(params, pcm_16k_mono)
        .context("whisper transcription failed")?;

    let mut text = String::new();
    for segment in state.as_iter() {
        if let Ok(segment_text) = segment.to_str() {
            if !text.is_empty() {
                text.push(' ');
            }
            text.push_str(segment_text.trim());
        }
    }

    Ok(text.trim().to_string())
}

/// Convenience for a README/setup check: true if a model file exists at
/// [`default_model_path`] (or `OTTO_WHISPER_MODEL`).
pub fn model_available() -> bool {
    default_model_path().map(|p| p.exists()).unwrap_or(false)
}

/// True if `path` looks like a real GGML model file (exists and is
/// non-trivially sized) - a cheap sanity check, not a format validator.
pub fn looks_like_model_file(path: &Path) -> bool {
    std::fs::metadata(path)
        .map(|m| m.len() > 1_000_000)
        .unwrap_or(false)
}
