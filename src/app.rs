use std::collections::VecDeque;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Child, Stdio};
use std::sync::{Arc, Condvar, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use anyhow::{Context, Result, anyhow};
use chrono::Local;

use crate::capture::CaptureProcess;
use crate::platform::{
    AudioSource, discover_audio_sources, print_sources_table, resolve_audio_source,
};
use crate::session::{SessionWriter, create_session_title};
use crate::signals;
use crate::transcribe::{WhisperConfig, transcribe_wav_file};
use crate::util::{format_duration, format_duration_ms};
use crate::vad::{SpeechSegment, StreamingVad, VadConfig};
use crate::wav::write_wav_file;

#[derive(Clone, Debug)]
pub struct StartConfig {
    pub ffmpeg_bin: PathBuf,
    pub whisper_bin: PathBuf,
    pub model_path: PathBuf,
    pub model_label: String,
    pub language: String,
    pub quality: String,
    pub threads: usize,
    pub chunk_seconds: u64,
    pub output_root: PathBuf,
    pub record_audio: bool,
    pub title: Option<String>,
    pub source_query: String,
}

pub fn list_sources(ffmpeg_bin: &Path) -> Result<Vec<AudioSource>> {
    let sources = discover_audio_sources(ffmpeg_bin)?;
    print_sources_table(&sources);
    Ok(sources)
}

pub fn run_session(config: StartConfig) -> Result<()> {
    if !config.model_path.exists() {
        return Err(anyhow!(
            "model '{}' is not installed or is not readable",
            config.model_path.display()
        ));
    }

    signals::install();

    let started_at = Local::now();
    let session_start = Instant::now();
    let sources = discover_audio_sources(&config.ffmpeg_bin)?;
    let source = resolve_audio_source(&sources, &config.source_query)?;
    let title = create_session_title(source, config.title.as_deref());

    println!("Lecture");
    println!("────────────────────────────────");
    println!();
    println!("Source      {}", source.display_name());
    println!("Model       {}", config.model_label);
    println!("Language    {}", human_language_name(&config.language));
    println!("Threads     {}", config.threads);
    println!("Audio       16 kHz mono");
    println!(
        "Recording   {}",
        if config.record_audio {
            "enabled"
        } else {
            "disabled"
        }
    );
    println!();
    println!("● Listening");
    println!();

    let session_writer = SessionWriter::new(
        &config.output_root,
        &title,
        started_at,
        &config.language,
        &config.model_label,
        config.threads,
        &config.quality,
        config.chunk_seconds,
        config.record_audio,
        &backend_name(),
        source,
    )?;

    let session_dir = session_writer.session_dir().to_path_buf();
    let scratch_dir = session_dir.join("scratch");
    fs::create_dir_all(&scratch_dir)?;

    let transcribe_config = WhisperConfig {
        whisper_bin: config.whisper_bin.clone(),
        model_path: config.model_path.clone(),
        language: config.language.clone(),
        threads: config.threads,
    };

    validate_whisper_binary(&transcribe_config.whisper_bin)?;

    let queue = Arc::new(SegmentQueue::new(4));
    let worker_queue = queue.clone();
    let worker = thread::spawn(move || -> Result<()> {
        worker_loop(worker_queue, session_writer, transcribe_config, scratch_dir)
    });

    let capture_process = CaptureProcess::spawn(&config.ffmpeg_bin, source, 16_000, 1)?;
    let (child, mut stdout) = capture_process.into_parts();
    let child_handle: Arc<Mutex<Option<Child>>> = Arc::new(Mutex::new(Some(child)));
    let killer = spawn_killer(child_handle.clone());

    let frame_ms = 20u64;
    let vad_cfg = VadConfig {
        max_segment_ms: config.chunk_seconds.saturating_mul(1000) as u32,
        ..VadConfig::default()
    };
    let mut vad = StreamingVad::new(16_000, 1, vad_cfg);
    let frame_bytes = vad.frame_bytes();
    let mut pending = Vec::with_capacity(frame_bytes * 8);
    let mut offset = 0usize;
    let mut frame_index = 0u64;
    let mut capture_buffer = [0u8; 8192];

    loop {
        if signals::stop_requested() {
            break;
        }

        let read = stdout.read(&mut capture_buffer)?;
        if read == 0 {
            break;
        }

        pending.extend_from_slice(&capture_buffer[..read]);

        while pending.len().saturating_sub(offset) >= frame_bytes {
            let frame = &pending[offset..offset + frame_bytes];
            if let Some(segment) = vad.push_frame(frame, frame_index * frame_ms) {
                if queue.push_segment(segment) {
                    eprintln!("Warning: transcription queue was full; dropped the oldest segment.");
                }
            }

            offset += frame_bytes;
            frame_index += 1;
        }

        if offset > frame_bytes * 32 {
            pending.drain(..offset);
            offset = 0;
        }
    }

    if let Some(segment) = vad.flush(frame_index * frame_ms) {
        if queue.push_segment(segment) {
            eprintln!("Warning: transcription queue was full; dropped the oldest segment.");
        }
    }

    let duration_seconds = session_start.elapsed().as_secs();
    let dropped_segments = queue.dropped_segments();
    queue.push_finish(duration_seconds, dropped_segments);
    queue.close();

    if let Some(mut child) = child_handle.lock().expect("poisoned capture mutex").take() {
        let _ = child.kill();
        let _ = child.wait();
    }

    let worker_result = worker
        .join()
        .map_err(|_| anyhow!("transcription worker panicked"))?;
    signals::request_stop();
    let _ = killer.join();
    worker_result?;

    println!("Session finished.");
    println!();
    println!("Duration        {}", format_duration(duration_seconds));
    println!("Saved:");
    println!("{}", session_dir.join("transcript.md").display());
    println!("{}", session_dir.join("session.json").display());

    Ok(())
}

fn worker_loop(
    queue: Arc<SegmentQueue>,
    mut session_writer: SessionWriter,
    config: WhisperConfig,
    scratch_dir: PathBuf,
) -> Result<()> {
    let mut speech_ms_total = 0u64;

    while let Some(item) = queue.pop() {
        match item {
            QueueItem::Segment(segment) => {
                speech_ms_total = speech_ms_total.saturating_add(segment.speech_ms);
                let wav_path = scratch_dir.join(format!(
                    "segment-{}-{}.wav",
                    segment.start_ms, segment.end_ms
                ));
                write_wav_file(&wav_path, &segment.pcm, 16_000, 1)?;

                match transcribe_wav_file(&wav_path, &scratch_dir, &config) {
                    Ok(text) => {
                        if !text.trim().is_empty() {
                            println!("[{}]", format_duration_ms(segment.start_ms));
                            println!("{}", text.trim());
                            println!();
                            let _ = session_writer.append_segment(&segment, &text)?;
                        }
                    }
                    Err(err) => {
                        eprintln!(
                            "Warning: failed to transcribe chunk starting at {}: {err:#}",
                            format_duration_ms(segment.start_ms)
                        );
                    }
                }
            }
            QueueItem::Finish {
                duration_seconds,
                dropped_segments,
            } => {
                let _metadata = session_writer.finish(
                    duration_seconds,
                    speech_ms_total / 1000,
                    dropped_segments,
                )?;
                return Ok(());
            }
        }
    }

    Err(anyhow!("transcription queue ended before finish signal"))
}

fn validate_whisper_binary(binary: &Path) -> Result<()> {
    let probe = std::process::Command::new(binary)
        .arg("-h")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();

    match probe {
        Ok(_) => Ok(()),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Err(anyhow!(
            "whisper binary '{}' was not found",
            binary.display()
        )),
        Err(err) => Err(err).context("failed to probe whisper binary"),
    }
}

fn human_language_name(language: &str) -> String {
    match language {
        "es" => "Spanish".to_string(),
        "en" => "English".to_string(),
        other => other.to_string(),
    }
}

fn backend_name() -> String {
    if cfg!(target_os = "windows") {
        "windows-wasapi+ffmpeg+whisper.cpp".to_string()
    } else {
        "linux-pipewire+pulse+ffmpeg+whisper.cpp".to_string()
    }
}

fn spawn_killer(child: Arc<Mutex<Option<Child>>>) -> thread::JoinHandle<()> {
    thread::spawn(move || {
        while !signals::stop_requested() {
            thread::sleep(Duration::from_millis(100));
        }

        if let Some(mut child) = child.lock().expect("poisoned capture mutex").take() {
            let _ = child.kill();
            let _ = child.wait();
        }
    })
}

#[derive(Clone)]
enum QueueItem {
    Segment(SpeechSegment),
    Finish {
        duration_seconds: u64,
        dropped_segments: usize,
    },
}

struct SegmentQueue {
    inner: Mutex<SegmentQueueInner>,
    wake: Condvar,
}

struct SegmentQueueInner {
    items: VecDeque<QueueItem>,
    closed: bool,
    dropped_segments: usize,
    capacity: usize,
}

impl SegmentQueue {
    fn new(capacity: usize) -> Self {
        Self {
            inner: Mutex::new(SegmentQueueInner {
                items: VecDeque::new(),
                closed: false,
                dropped_segments: 0,
                capacity,
            }),
            wake: Condvar::new(),
        }
    }

    fn push_segment(&self, segment: SpeechSegment) -> bool {
        let mut inner = self.inner.lock().expect("poisoned queue mutex");
        if inner.closed {
            return false;
        }

        let mut dropped = false;
        while inner.items.len() >= inner.capacity {
            if let Some(QueueItem::Segment(_)) = inner.items.pop_front() {
                inner.dropped_segments += 1;
                dropped = true;
            } else {
                break;
            }
        }

        inner.items.push_back(QueueItem::Segment(segment));
        self.wake.notify_one();
        dropped
    }

    fn push_finish(&self, duration_seconds: u64, dropped_segments: usize) {
        let mut inner = self.inner.lock().expect("poisoned queue mutex");
        inner.items.push_back(QueueItem::Finish {
            duration_seconds,
            dropped_segments,
        });
        self.wake.notify_all();
    }

    fn pop(&self) -> Option<QueueItem> {
        let mut inner = self.inner.lock().expect("poisoned queue mutex");
        loop {
            if let Some(item) = inner.items.pop_front() {
                return Some(item);
            }

            if inner.closed {
                return None;
            }

            inner = self.wake.wait(inner).expect("poisoned queue mutex");
        }
    }

    fn close(&self) {
        let mut inner = self.inner.lock().expect("poisoned queue mutex");
        inner.closed = true;
        self.wake.notify_all();
    }

    fn dropped_segments(&self) -> usize {
        self.inner
            .lock()
            .expect("poisoned queue mutex")
            .dropped_segments
    }
}
