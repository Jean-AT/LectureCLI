use std::collections::VecDeque;

#[derive(Clone, Copy, Debug)]
pub struct VadConfig {
    pub frame_ms: u32,
    pub start_ms: u32,
    pub stop_ms: u32,
    pub preroll_ms: u32,
    pub max_segment_ms: u32,
    pub min_segment_ms: u32,
    pub silence_threshold: i16,
}

impl Default for VadConfig {
    fn default() -> Self {
        Self {
            frame_ms: 20,
            start_ms: 160,
            stop_ms: 700,
            preroll_ms: 200,
            max_segment_ms: 25_000,
            min_segment_ms: 500,
            silence_threshold: 500,
        }
    }
}

#[derive(Clone, Debug)]
pub struct SpeechSegment {
    pub start_ms: u64,
    pub end_ms: u64,
    pub speech_ms: u64,
    pub pcm: Vec<u8>,
}

pub struct StreamingVad {
    cfg: VadConfig,
    frame_bytes: usize,
    max_segment_bytes: usize,
    min_segment_bytes: usize,
    preroll_frames: usize,
    ring: VecDeque<Vec<u8>>,
    active: bool,
    speech_run_frames: u32,
    silence_run_frames: u32,
    segment_start_ms: u64,
    current_pcm: Vec<u8>,
    current_speech_ms: u64,
}

impl StreamingVad {
    pub fn new(sample_rate: u32, channels: u16, cfg: VadConfig) -> Self {
        let frame_samples = (sample_rate as u64 * cfg.frame_ms as u64 / 1000) as usize;
        let frame_bytes = frame_samples * channels as usize * 2;
        let max_segment_bytes =
            (sample_rate as u64 * channels as u64 * 2 * cfg.max_segment_ms as u64 / 1000) as usize;
        let min_segment_bytes =
            (sample_rate as u64 * channels as u64 * 2 * cfg.min_segment_ms as u64 / 1000) as usize;
        let preroll_frames = (cfg.preroll_ms / cfg.frame_ms).max(1) as usize;

        Self {
            cfg,
            frame_bytes,
            max_segment_bytes,
            min_segment_bytes,
            preroll_frames,
            ring: VecDeque::with_capacity(preroll_frames + 1),
            active: false,
            speech_run_frames: 0,
            silence_run_frames: 0,
            segment_start_ms: 0,
            current_pcm: Vec::new(),
            current_speech_ms: 0,
        }
    }

    pub fn frame_bytes(&self) -> usize {
        self.frame_bytes
    }

    pub fn push_frame(&mut self, frame: &[u8], frame_start_ms: u64) -> Option<SpeechSegment> {
        debug_assert_eq!(frame.len(), self.frame_bytes);

        self.ring.push_back(frame.to_vec());
        while self.ring.len() > self.preroll_frames {
            self.ring.pop_front();
        }

        let speech = is_speech_frame(frame, self.cfg.silence_threshold);

        if !self.active {
            if speech {
                self.speech_run_frames = self.speech_run_frames.saturating_add(1);
            } else {
                self.speech_run_frames = 0;
            }

            if self.speech_run_frames.saturating_mul(self.cfg.frame_ms) >= self.cfg.start_ms {
                self.active = true;
                self.segment_start_ms = frame_start_ms.saturating_sub(self.cfg.preroll_ms as u64);
                self.current_pcm.clear();
                for cached in &self.ring {
                    self.current_pcm.extend_from_slice(cached);
                }
                self.current_speech_ms = self.speech_run_frames as u64 * self.cfg.frame_ms as u64;
                self.silence_run_frames = 0;
            }

            return None;
        }

        self.current_pcm.extend_from_slice(frame);
        if speech {
            self.current_speech_ms = self
                .current_speech_ms
                .saturating_add(self.cfg.frame_ms as u64);
            self.silence_run_frames = 0;
        } else {
            self.silence_run_frames = self.silence_run_frames.saturating_add(1);
        }

        if self.current_pcm.len() >= self.max_segment_bytes {
            return self.finish_segment(frame_start_ms + self.cfg.frame_ms as u64);
        }

        if self.silence_run_frames.saturating_mul(self.cfg.frame_ms) >= self.cfg.stop_ms {
            return self.finish_segment(frame_start_ms + self.cfg.frame_ms as u64);
        }

        None
    }

    pub fn flush(&mut self, end_ms: u64) -> Option<SpeechSegment> {
        if self.active {
            return self.finish_segment(end_ms);
        }

        None
    }

    fn finish_segment(&mut self, end_ms: u64) -> Option<SpeechSegment> {
        let pcm = std::mem::take(&mut self.current_pcm);
        let start_ms = self.segment_start_ms;
        let speech_ms = self.current_speech_ms;

        self.active = false;
        self.speech_run_frames = 0;
        self.silence_run_frames = 0;
        self.current_speech_ms = 0;

        if pcm.len() >= self.min_segment_bytes {
            Some(SpeechSegment {
                start_ms,
                end_ms,
                speech_ms,
                pcm,
            })
        } else {
            None
        }
    }
}

fn is_speech_frame(frame: &[u8], threshold: i16) -> bool {
    let mut sum = 0u64;
    let mut samples = 0u64;

    for chunk in frame.chunks_exact(2) {
        let sample = i16::from_le_bytes([chunk[0], chunk[1]]);
        sum = sum.saturating_add(sample.unsigned_abs() as u64);
        samples += 1;
    }

    if samples == 0 {
        return false;
    }

    (sum / samples) as i64 >= threshold as i64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vad_frame_size_matches_expected_pcm_layout() {
        let vad = StreamingVad::new(16_000, 1, VadConfig::default());
        assert_eq!(vad.frame_bytes(), 640);
    }
}
