use std::io;
use std::path::Path;
use std::process::{Command, Stdio};

use anyhow::{Context, Result, anyhow};

#[derive(Clone, Debug)]
pub struct AudioSource {
    pub id: usize,
    pub application: String,
    pub stream: String,
    pub capture_spec: String,
    pub backend: String,
}

impl AudioSource {
    pub fn display_name(&self) -> String {
        format!("{}/{}", self.application, self.stream)
    }
}

pub fn discover_audio_sources(ffmpeg_bin: &Path) -> Result<Vec<AudioSource>> {
    let mut sources = if cfg!(target_os = "windows") {
        discover_windows_sources(ffmpeg_bin)?
    } else {
        discover_unix_sources()?
    };

    if sources.is_empty() {
        sources.push(AudioSource {
            id: 1,
            application: if cfg!(target_os = "windows") {
                "WASAPI".to_string()
            } else {
                "PipeWire".to_string()
            },
            stream: "default".to_string(),
            capture_spec: "default".to_string(),
            backend: if cfg!(target_os = "windows") {
                "wasapi".to_string()
            } else {
                "pulse".to_string()
            },
        });
    }

    for (index, source) in sources.iter_mut().enumerate() {
        source.id = index + 1;
    }

    Ok(sources)
}

pub fn resolve_audio_source<'a>(
    sources: &'a [AudioSource],
    query: &str,
) -> Result<&'a AudioSource> {
    if let Ok(index) = query.parse::<usize>() {
        return sources
            .iter()
            .find(|source| source.id == index)
            .ok_or_else(|| anyhow!("audio source id {index} was not found"));
    }

    let query_lower = query.to_lowercase();
    sources
        .iter()
        .find(|source| {
            source.stream.eq_ignore_ascii_case(query)
                || source.display_name().to_lowercase().contains(&query_lower)
                || source.capture_spec.to_lowercase().contains(&query_lower)
        })
        .ok_or_else(|| anyhow!("audio source '{query}' was not found"))
}

pub fn print_sources_table(sources: &[AudioSource]) {
    println!("Available audio sources\n");
    println!("{:<4} {:<16} STREAM", "ID", "APPLICATION");
    println!("────────────────────────────────────────────────────────");
    for source in sources {
        println!(
            "{:<4} {:<16} {}",
            source.id, source.application, source.stream
        );
    }
}

pub fn build_ffmpeg_capture_command(
    ffmpeg_bin: &Path,
    source: &AudioSource,
    sample_rate: u32,
    channels: u16,
) -> Command {
    let mut command = Command::new(ffmpeg_bin);
    command
        .arg("-hide_banner")
        .arg("-loglevel")
        .arg("error")
        .arg("-nostdin");

    if cfg!(target_os = "windows") {
        command
            .arg("-f")
            .arg("wasapi")
            .arg("-loopback")
            .arg("1")
            .arg("-i")
            .arg(&source.capture_spec);
    } else {
        command
            .arg("-f")
            .arg("pulse")
            .arg("-i")
            .arg(&source.capture_spec);
    }

    command
        .arg("-ac")
        .arg(channels.to_string())
        .arg("-ar")
        .arg(sample_rate.to_string())
        .arg("-f")
        .arg("s16le")
        .arg("pipe:1")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    command
}

fn discover_unix_sources() -> Result<Vec<AudioSource>> {
    match Command::new("pactl")
        .arg("list")
        .arg("short")
        .arg("sources")
        .output()
    {
        Ok(output) if output.status.success() => parse_pactl_sources(&output.stdout),
        Ok(output) => Err(anyhow::anyhow!(
            "pactl failed while listing sources: {}",
            String::from_utf8_lossy(&output.stderr)
        )),
        Err(err) if err.kind() == io::ErrorKind::NotFound => {
            Err(anyhow!("pactl is not available on this system"))
        }
        Err(err) => Err(err).context("failed to invoke pactl"),
    }
}

fn parse_pactl_sources(stdout: &[u8]) -> Result<Vec<AudioSource>> {
    let text = String::from_utf8_lossy(stdout);
    let mut sources = Vec::new();

    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        let cols: Vec<&str> = trimmed.split('\t').collect();
        if cols.len() < 2 {
            continue;
        }

        let name = cols[1].trim().to_string();
        let stream = name.clone();
        let application = if name.contains(".monitor") {
            "PipeWire monitor".to_string()
        } else {
            "PipeWire source".to_string()
        };

        sources.push(AudioSource {
            id: 0,
            application,
            stream,
            capture_spec: name,
            backend: "pulse".to_string(),
        });
    }

    Ok(sources)
}

fn discover_windows_sources(ffmpeg_bin: &Path) -> Result<Vec<AudioSource>> {
    let output = Command::new(ffmpeg_bin)
        .arg("-hide_banner")
        .arg("-list_devices")
        .arg("true")
        .arg("-f")
        .arg("wasapi")
        .arg("-i")
        .arg("dummy")
        .output()
        .context("failed to invoke ffmpeg for WASAPI device discovery")?;

    let text = format!(
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let mut sources = Vec::new();
    for line in text.lines() {
        let Some(start) = line.find('"') else {
            continue;
        };
        let Some(end) = line[start + 1..].find('"') else {
            continue;
        };
        let name = line[start + 1..start + 1 + end].trim();
        if name.is_empty() || name.eq_ignore_ascii_case("default") {
            continue;
        }

        sources.push(AudioSource {
            id: 0,
            application: "WASAPI loopback".to_string(),
            stream: name.to_string(),
            capture_spec: name.to_string(),
            backend: "wasapi".to_string(),
        });
    }

    if !output.status.success() && sources.is_empty() {
        return Err(anyhow!(
            "ffmpeg failed while listing WASAPI devices: {}",
            text
        ));
    }

    Ok(sources)
}
