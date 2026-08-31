use std::fs::{self, File};
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};

use anyhow::Result;
use chrono::{DateTime, Local};
use serde::Serialize;

use crate::platform::AudioSource;
use crate::util::{
    format_duration, format_duration_ms, format_local_date, format_local_time, slugify,
};
use crate::vad::SpeechSegment;

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AudioSourceMetadata {
    pub application: String,
    pub name: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionMetadata {
    pub version: u32,
    pub title: String,
    pub started_at: String,
    pub duration_seconds: u64,
    pub speech_seconds: u64,
    pub words: usize,
    pub language: String,
    pub model: String,
    pub threads: usize,
    pub quality: String,
    pub chunk_seconds: u64,
    pub record_audio: bool,
    pub backend: String,
    pub audio_source: AudioSourceMetadata,
    pub transcript_path: String,
    pub session_directory: String,
    pub dropped_segments: usize,
}

pub struct SessionWriter {
    session_dir: PathBuf,
    transcript_path: PathBuf,
    session_path: PathBuf,
    transcript: BufWriter<File>,
    started_at: DateTime<Local>,
    title: String,
    language: String,
    model: String,
    threads: usize,
    quality: String,
    chunk_seconds: u64,
    record_audio: bool,
    backend: String,
    source: AudioSourceMetadata,
    words: usize,
}

impl SessionWriter {
    pub fn new(
        output_root: &Path,
        title: &str,
        started_at: DateTime<Local>,
        language: &str,
        model: &str,
        threads: usize,
        quality: &str,
        chunk_seconds: u64,
        record_audio: bool,
        backend: &str,
        source: &AudioSource,
    ) -> Result<Self> {
        fs::create_dir_all(output_root)?;
        let session_name = format!("{}-{}", format_local_date(started_at), slugify(title));
        let session_dir = output_root.join(session_name);
        fs::create_dir_all(&session_dir)?;

        let transcript_path = session_dir.join("transcript.md");
        let session_path = session_dir.join("session.json");
        let transcript_file = File::create(&transcript_path)?;
        let mut transcript = BufWriter::new(transcript_file);

        writeln!(transcript, "# {}", title)?;
        writeln!(transcript)?;
        writeln!(transcript, "Date: {}", format_local_date(started_at))?;
        writeln!(transcript, "Start: {}", format_local_time(started_at))?;
        writeln!(transcript, "Language: {}", language)?;
        writeln!(transcript, "Model: {}", model)?;
        writeln!(transcript, "Threads: {}", threads)?;
        writeln!(transcript, "Quality: {}", quality)?;
        writeln!(transcript, "Source: {}", source.display_name())?;
        writeln!(
            transcript,
            "Recording: {}",
            if record_audio { "enabled" } else { "disabled" }
        )?;
        writeln!(transcript)?;
        writeln!(transcript, "---")?;
        writeln!(transcript)?;
        transcript.flush()?;

        Ok(Self {
            session_dir,
            transcript_path,
            session_path,
            transcript,
            started_at,
            title: title.to_string(),
            language: language.to_string(),
            model: model.to_string(),
            threads,
            quality: quality.to_string(),
            chunk_seconds,
            record_audio,
            backend: backend.to_string(),
            source: AudioSourceMetadata {
                application: source.application.clone(),
                name: source.stream.clone(),
            },
            words: 0,
        })
    }

    pub fn session_dir(&self) -> &Path {
        &self.session_dir
    }

    pub fn append_segment(&mut self, segment: &SpeechSegment, text: &str) -> Result<usize> {
        if text.trim().is_empty() {
            return Ok(0);
        }

        let word_count = text.split_whitespace().count();
        self.words += word_count;

        writeln!(
            self.transcript,
            "## {}",
            format_duration_ms(segment.start_ms)
        )?;
        writeln!(self.transcript)?;
        writeln!(self.transcript, "{}", text.trim())?;
        writeln!(self.transcript)?;
        self.transcript.flush()?;

        Ok(word_count)
    }

    pub fn finish(
        mut self,
        duration_seconds: u64,
        speech_seconds: u64,
        dropped_segments: usize,
    ) -> Result<SessionMetadata> {
        writeln!(self.transcript, "---")?;
        writeln!(self.transcript)?;
        writeln!(self.transcript, "## Session Summary")?;
        writeln!(self.transcript)?;
        writeln!(
            self.transcript,
            "Duration: {}",
            format_duration(duration_seconds)
        )?;
        writeln!(
            self.transcript,
            "Speech: {}",
            format_duration(speech_seconds)
        )?;
        writeln!(self.transcript, "Words: {}", self.words)?;
        writeln!(self.transcript, "Dropped segments: {}", dropped_segments)?;
        writeln!(self.transcript, "Saved:")?;
        writeln!(self.transcript, "{}", self.transcript_path.display())?;
        writeln!(self.transcript, "{}", self.session_path.display())?;
        self.transcript.flush()?;

        let metadata = SessionMetadata {
            version: 1,
            title: self.title,
            started_at: self.started_at.to_rfc3339(),
            duration_seconds,
            speech_seconds,
            words: self.words,
            language: self.language,
            model: self.model,
            threads: self.threads,
            quality: self.quality,
            chunk_seconds: self.chunk_seconds,
            record_audio: self.record_audio,
            backend: self.backend,
            audio_source: self.source,
            transcript_path: self.transcript_path.display().to_string(),
            session_directory: self.session_dir.display().to_string(),
            dropped_segments,
        };

        let metadata_text = serde_json::to_string_pretty(&metadata)?;
        fs::write(&self.session_path, metadata_text)?;

        Ok(metadata)
    }
}

pub fn create_session_title(source: &AudioSource, explicit: Option<&str>) -> String {
    explicit
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| source.stream.clone())
}

pub fn metadata_backend_name(platform: &str, record_audio: bool) -> String {
    format!("{platform}{}", if record_audio { "+record" } else { "" })
}
