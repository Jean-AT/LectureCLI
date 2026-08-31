# LectureCLI

LectureCLI is a local-first command-line tool for capturing class audio and turning it into a transcript on your machine.

It is built for a simple workflow:

1. Find an audio source.
2. Start a transcription session.
3. Let it run during class.
4. Stop it with `Ctrl+C`.
5. Read the saved transcript later.

## What it does

- Captures audio from the selected source
- Converts audio to 16 kHz mono PCM
- Uses local Whisper transcription through `whisper.cpp`
- Writes a markdown transcript and session metadata
- Runs on Linux with PipeWire and on Windows with WASAPI loopback

## What it is for

This project is for virtual classes, lectures, and other long audio sessions where you want a local transcript without sending audio to a cloud service.

The current MVP is tuned for:

- Spanish classes
- low resource usage
- long sessions
- simple CLI usage

## Install

If you have this repo next to a local `whisper.cpp` clone, run:

```bash
bash install.sh
```

That script will:

- install `lecture`
- build `whisper.cpp` if needed
- download the default Whisper model if needed
- create a launcher in `~/.local/bin`
- add `~/.local/bin` and `~/.cargo/bin` to your shell PATH for new terminals

## Requirements

- `cargo`
- `cmake`
- `ffmpeg`
- a local `whisper.cpp` clone
- a Whisper model file, usually `ggml-base.bin`

### What each one is for

- `cargo`: installs and runs `lecture`
- `cmake`: builds `whisper.cpp`
- `ffmpeg`: captures audio and feeds it to Whisper
- `whisper.cpp`: provides the local transcription binary
- `ggml-base.bin`: the default Whisper model used by the MVP

## Quick start

List available sources:

```bash
lecture devices
```

Start a session:

```bash
lecture start 3 clase-fisica2
```

That command uses the current defaults:

- language: `es`
- quality: `balanced`
- threads: `2`

If you want to override them, you can still pass flags:

```bash
lecture start 3 clase-fisica2 --language es --quality balanced --threads 2
```

## Output

Each session is saved under `sessions/` with:

- `transcript.md`
- `session.json`

Example:

```text
sessions/2026-08-30-clase-fisica2/
  transcript.md
  session.json
```

## Notes

- On Linux, source discovery uses PipeWire-compatible sources through `ffmpeg`.
- On Windows, capture uses WASAPI loopback through `ffmpeg`.
- Audio is not stored by default.
- The transcript stays local.

## Project goal

The goal is not live subtitles.

The goal is a lightweight CLI you can start at the beginning of class and trust until the class ends.
