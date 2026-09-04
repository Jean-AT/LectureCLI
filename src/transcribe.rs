use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};

use anyhow::{Context, Result, anyhow};

use crate::util::normalize_whitespace;

static TRANSCRIBE_COUNTER: AtomicUsize = AtomicUsize::new(1);

#[derive(Clone, Debug)]
pub struct WhisperConfig {
    pub whisper_bin: PathBuf,
    pub model_path: PathBuf,
    pub language: String,
    pub threads: usize,
    pub no_speech_threshold: f32,
    pub keep_artifacts: bool,
}

pub fn transcribe_wav_file(
    wav_path: &Path,
    scratch_dir: &Path,
    config: &WhisperConfig,
) -> Result<String> {
    fs::create_dir_all(scratch_dir)?;

    let stem = format!(
        "whisper-{}-{}",
        std::process::id(),
        TRANSCRIBE_COUNTER.fetch_add(1, Ordering::SeqCst)
    );
    let output_base = scratch_dir.join(stem);
    let output_txt = output_base.with_extension("txt");

    let output = Command::new(&config.whisper_bin)
        .arg("-m")
        .arg(&config.model_path)
        .arg("-f")
        .arg(wav_path)
        .arg("-of")
        .arg(&output_base)
        .arg("-otxt")
        .arg("-np")
        .arg("-nt")
        .arg("-l")
        .arg(&config.language)
        .arg("-t")
        .arg(config.threads.to_string())
        .arg("-nth")
        .arg(config.no_speech_threshold.to_string())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .with_context(|| {
            format!(
                "failed to invoke whisper binary '{}'",
                config.whisper_bin.display()
            )
        })?;

    if !output.status.success() {
        let mut details = String::from_utf8_lossy(&output.stderr).to_string();
        if details.is_empty() {
            details = String::from_utf8_lossy(&output.stdout).to_string();
        }
        return Err(anyhow!(
            "whisper transcription failed (status {})\n  whisper binary: {}\n  model: {}\n  input: {}\n  details: {}",
            output.status,
            config.whisper_bin.display(),
            config.model_path.display(),
            wav_path.display(),
            details.trim(),
        ));
    }

    let text = if output_txt.exists() {
        fs::read_to_string(&output_txt)?
    } else {
        String::from_utf8_lossy(&output.stdout).to_string()
    };

    let cleaned = normalize_whitespace(&text);
    if !config.keep_artifacts {
        let _ = fs::remove_file(&output_txt);
        let _ = fs::remove_file(wav_path);
    }

    Ok(cleaned)
}
