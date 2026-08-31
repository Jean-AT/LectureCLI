use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::Path;

use anyhow::{Result, anyhow};

pub fn write_wav_file(
    path: &Path,
    pcm_s16le: &[u8],
    sample_rate: u32,
    channels: u16,
) -> Result<()> {
    if pcm_s16le.len() % 2 != 0 {
        return Err(anyhow!(
            "pcm buffer length must be aligned to 16-bit samples"
        ));
    }

    let mut writer = BufWriter::new(File::create(path)?);
    let data_len = pcm_s16le.len() as u32;
    let byte_rate = sample_rate * channels as u32 * 2;
    let block_align = channels * 2;
    let riff_size = 36 + data_len;

    writer.write_all(b"RIFF")?;
    writer.write_all(&riff_size.to_le_bytes())?;
    writer.write_all(b"WAVE")?;
    writer.write_all(b"fmt ")?;
    writer.write_all(&16u32.to_le_bytes())?;
    writer.write_all(&1u16.to_le_bytes())?;
    writer.write_all(&channels.to_le_bytes())?;
    writer.write_all(&sample_rate.to_le_bytes())?;
    writer.write_all(&byte_rate.to_le_bytes())?;
    writer.write_all(&block_align.to_le_bytes())?;
    writer.write_all(&16u16.to_le_bytes())?;
    writer.write_all(b"data")?;
    writer.write_all(&data_len.to_le_bytes())?;
    writer.write_all(pcm_s16le)?;
    writer.flush()?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn wav_header_is_written() {
        let path = std::env::temp_dir().join("lecture-test.wav");
        let pcm = [0u8; 4];
        write_wav_file(&path, &pcm, 16_000, 1).unwrap();
        let bytes = fs::read(&path).unwrap();
        assert_eq!(&bytes[..4], b"RIFF");
        assert_eq!(&bytes[8..12], b"WAVE");
        let _ = fs::remove_file(path);
    }
}
