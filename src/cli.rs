use std::env;
use std::path::Path;
use std::path::PathBuf;

use anyhow::{Context, Result, anyhow, bail};

use crate::app::{StartConfig, list_sources, run_session};

pub fn run() -> Result<()> {
    let mut args = env::args().skip(1);
    let Some(command) = args.next() else {
        print_help();
        return Ok(());
    };

    match command.as_str() {
        "help" | "--help" | "-h" => {
            print_help();
            Ok(())
        }
        "devices" => match parse_devices_args(args.collect()) {
            Ok(options) => {
                let _ = list_sources(&options.ffmpeg_bin)?;
                Ok(())
            }
            Err(err) if err.to_string() == "help requested" => Ok(()),
            Err(err) => Err(err),
        },
        "start" => match parse_start_args(args.collect()) {
            Ok(options) => run_session(options),
            Err(err) if err.to_string() == "help requested" => Ok(()),
            Err(err) => Err(err),
        },
        _ => {
            eprintln!("Error: unknown command '{command}'\n");
            print_help();
            Ok(())
        }
    }
}

fn print_help() {
    println!(
        "Lecture\n\nLocal-first virtual class transcription.\n\nUsage:\n    lecture <COMMAND>\n\nCommands:\n    devices     List available audio sources\n    start       Start a transcription session\n    help        Print help\n\nExamples:\n    lecture devices\n    lecture start 1 clase-fisica2\n"
    );
}

struct DevicesArgs {
    ffmpeg_bin: PathBuf,
}

fn parse_devices_args(args: Vec<String>) -> Result<DevicesArgs> {
    let mut ffmpeg_bin = PathBuf::from("ffmpeg");

    let mut iter = args.into_iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--ffmpeg-bin" => {
                ffmpeg_bin = PathBuf::from(next_value(&mut iter, "--ffmpeg-bin")?);
            }
            "--help" | "-h" => {
                print_help();
                return Err(anyhow!("help requested"));
            }
            other => {
                return Err(anyhow!("unexpected argument '{other}' for devices"));
            }
        }
    }

    Ok(DevicesArgs { ffmpeg_bin })
}

fn parse_start_args(args: Vec<String>) -> Result<StartConfig> {
    let default_threads = 2;
    let mut quality = "balanced".to_string();
    let mut model_label = String::new();
    let mut model_path = PathBuf::new();
    let mut language = "es".to_string();
    let mut threads = default_threads;
    let mut chunk_seconds = 20u64;
    let mut output_root = PathBuf::from("sessions");
    let mut record_audio = false;
    let mut title = None;
    let mut ffmpeg_bin = PathBuf::from("ffmpeg");
    let mut whisper_bin = default_whisper_bin();
    let mut source_query = None;
    let mut eco = false;

    let mut iter = args.into_iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--language" | "-l" => language = next_value(&mut iter, "--language")?,
            "--quality" => quality = next_value(&mut iter, "--quality")?,
            "--model" => {
                model_path = PathBuf::from(next_value(&mut iter, "--model")?);
            }
            "--threads" | "-t" => {
                threads = next_value(&mut iter, "--threads")?
                    .parse()
                    .with_context(|| "threads must be a positive integer")?;
            }
            "--chunk-size" => {
                chunk_seconds = next_value(&mut iter, "--chunk-size")?
                    .parse()
                    .with_context(|| "chunk size must be an integer number of seconds")?;
            }
            "--output" => output_root = PathBuf::from(next_value(&mut iter, "--output")?),
            "--record" => record_audio = true,
            "--title" => title = Some(next_value(&mut iter, "--title")?),
            "--ffmpeg-bin" => ffmpeg_bin = PathBuf::from(next_value(&mut iter, "--ffmpeg-bin")?),
            "--whisper-bin" => whisper_bin = PathBuf::from(next_value(&mut iter, "--whisper-bin")?),
            "--eco" => eco = true,
            "--help" | "-h" => {
                print_help();
                return Err(anyhow!("help requested"));
            }
            other if other.starts_with('-') => {
                return Err(anyhow!("unknown option '{other}'"));
            }
            other => {
                if source_query.is_none() {
                    source_query = Some(other.to_string());
                } else if title.is_none() {
                    title = Some(other.to_string());
                } else {
                    return Err(anyhow!("unexpected extra positional argument '{other}'"));
                }
            }
        }
    }

    if eco {
        quality = "balanced".to_string();
        threads = 2;
        chunk_seconds = 20;
        record_audio = false;
    }

    let (default_label, default_model_path) = quality_defaults(&quality)?;
    if model_path.as_os_str().is_empty() {
        model_path = default_model_path;
        model_label = default_label;
    } else if model_label.is_empty() {
        model_label = model_path
            .file_stem()
            .and_then(|value| value.to_str())
            .unwrap_or("custom-model")
            .to_string();
    }

    let source_query = source_query.ok_or_else(|| anyhow!("missing source id or name"))?;

    Ok(StartConfig {
        ffmpeg_bin,
        whisper_bin,
        model_path,
        model_label,
        language,
        quality,
        threads,
        chunk_seconds,
        output_root,
        record_audio,
        title,
        source_query,
    })
}

fn quality_defaults(quality: &str) -> Result<(String, PathBuf)> {
    match quality {
        "fast" => Ok(("tiny-q5".to_string(), default_model_path("tiny"))),
        "balanced" => Ok(("base-q5".to_string(), default_model_path("base"))),
        "accurate" => Ok(("small-q5".to_string(), default_model_path("small"))),
        other => bail!("unknown quality profile '{other}'"),
    }
}

fn default_whisper_bin() -> PathBuf {
    if let Ok(path) = env::var("LECTURE_WHISPER_BIN") {
        let path = PathBuf::from(path);
        if path.exists() {
            return path;
        }
    }

    if let Ok(dir) = env::var("LECTURE_WHISPER_CPP_DIR") {
        let base = PathBuf::from(dir);
        let candidates = [
            base.join("build/bin/whisper-cli"),
            base.join("build/bin/Release/whisper-cli.exe"),
        ];

        for candidate in candidates {
            if candidate.exists() {
                return candidate;
            }
        }
    }

    if let Ok(cwd) = env::current_dir() {
        let candidate = cwd.join("../whisper.cpp/build/bin/whisper-cli");
        if candidate.exists() {
            return candidate;
        }
        let candidate = cwd.join("../../whisper.cpp/build/bin/whisper-cli");
        if candidate.exists() {
            return candidate;
        }
        let candidate = cwd.join("../whisper.cpp/build/bin/Release/whisper-cli.exe");
        if candidate.exists() {
            return candidate;
        }
    }

    PathBuf::from("whisper-cli")
}

fn default_model_path(size: &str) -> PathBuf {
    if let Ok(path) = env::var("LECTURE_WHISPER_MODEL") {
        let path = PathBuf::from(path);
        if path.exists() {
            return path;
        }
    }

    if let Ok(dir) = env::var("LECTURE_WHISPER_MODEL_DIR") {
        let base = PathBuf::from(dir);
        let candidates = [
            base.join(format!("ggml-{size}.bin")),
            base.join(format!("for-tests-ggml-{size}.bin")),
        ];

        for candidate in candidates {
            if candidate.exists() {
                return candidate;
            }
        }
    }

    if let Ok(dir) = env::var("LECTURE_WHISPER_CPP_DIR") {
        let base = PathBuf::from(dir).join("models");
        let candidates = [
            base.join(format!("ggml-{size}.bin")),
            base.join(format!("for-tests-ggml-{size}.bin")),
        ];

        for candidate in candidates {
            if candidate.exists() {
                return candidate;
            }
        }
    }

    let candidates = [
        format!("../whisper.cpp/models/ggml-{size}.bin"),
        format!("../../whisper.cpp/models/ggml-{size}.bin"),
        format!("../whisper.cpp/models/for-tests-ggml-{size}.bin"),
        format!("../../whisper.cpp/models/for-tests-ggml-{size}.bin"),
    ];

    for candidate in candidates {
        let path = Path::new(&candidate).to_path_buf();
        if path.exists() {
            return path;
        }
    }

    PathBuf::from(format!("models/{size}-q5.bin"))
}

fn next_value(iter: &mut impl Iterator<Item = String>, flag: &str) -> Result<String> {
    iter.next()
        .ok_or_else(|| anyhow!("expected a value after {flag}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quality_defaults_map_to_expected_models() {
        let (label, path) = quality_defaults("balanced").unwrap();
        assert_eq!(label, "base-q5");
        assert_eq!(path, default_model_path("base"));
    }
}
