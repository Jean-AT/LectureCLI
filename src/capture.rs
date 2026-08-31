use std::io;
use std::path::Path;
use std::process::{Child, ChildStdout};

use anyhow::{Context, Result};

use crate::platform::{AudioSource, build_ffmpeg_capture_command};

pub struct CaptureProcess {
    child: Child,
    pub stdout: ChildStdout,
}

impl CaptureProcess {
    pub fn spawn(
        ffmpeg_bin: &Path,
        source: &AudioSource,
        sample_rate: u32,
        channels: u16,
    ) -> Result<Self> {
        let mut command = build_ffmpeg_capture_command(ffmpeg_bin, source, sample_rate, channels);
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

        Ok(Self { child, stdout })
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

    pub fn into_parts(self) -> (Child, ChildStdout) {
        (self.child, self.stdout)
    }
}
