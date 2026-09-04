use std::io;
use std::path::Path;
use std::process::{Child, ChildStderr, ChildStdout};

use anyhow::{Context, Result};

use crate::platform::{AudioSource, build_ffmpeg_capture_command};

pub struct CaptureProcess {
    child: Child,
    pub stdout: ChildStdout,
    pub stderr: ChildStderr,
}

impl CaptureProcess {
    pub fn spawn(
        ffmpeg_bin: &Path,
        source: &AudioSource,
        sample_rate: u32,
        channels: u16,
        input_gain: f32,
    ) -> Result<Self> {
        let mut command =
            build_ffmpeg_capture_command(ffmpeg_bin, source, sample_rate, channels, input_gain);
        let mut child = command.spawn().with_context(|| {
            format!(
                "failed to start ffmpeg capture for '{}'",
                source.display_name()
            )
        })?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| anyhow::anyhow!("ffmpeg capture process did not expose stdout"))?;
        let stderr = child
            .stderr
            .take()
            .ok_or_else(|| anyhow::anyhow!("ffmpeg capture process did not expose stderr"))?;

        Ok(Self {
            child,
            stdout,
            stderr,
        })
    }

    pub fn kill_and_wait(&mut self) -> io::Result<()> {
        let _ = self.child.kill();
        let _ = self.child.wait();
        Ok(())
    }

    pub fn wait(&mut self) -> io::Result<()> {
        let _ = self.child.wait();
        Ok(())
    }

    pub fn into_parts(self) -> (Child, ChildStdout, ChildStderr) {
        (self.child, self.stdout, self.stderr)
    }
}
