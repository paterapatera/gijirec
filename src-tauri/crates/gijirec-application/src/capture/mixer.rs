//! Dual-source alignment, level normalization, and mixing.

use std::collections::VecDeque;

/// Sample rate for mixed output (16 kHz mono).
pub const SAMPLE_RATE_HZ: u32 = 16_000;

/// 50 ms alignment window at 16 kHz.
const ALIGNMENT_SAMPLES: u64 = 800;

/// 200 ms RMS measurement window at 16 kHz.
const RMS_WINDOW_SAMPLES: usize = 3_200;

/// Maximum retained samples (~30 s at 16 kHz).
const MAX_RETENTION_SAMPLES: usize = 480_000;

/// Target RMS level (-20 dBFS).
const TARGET_RMS: f32 = 0.1;

/// Soft limiter ceiling.
const SOFT_LIMIT: f32 = 0.95;

/// Floor to avoid division by zero in gain calculation.
const MIN_RMS: f32 = 1e-8;

/// Mixes mic and system audio with timeline alignment and level normalization.
pub trait AudioMixer: Send {
    /// Pushes mic samples starting at `timeline_samples` on the unified 16 kHz timeline.
    fn push_mic(&mut self, samples: &[f32], timeline_samples: u64);
    /// Pushes system audio samples starting at `timeline_samples`.
    fn push_system(&mut self, samples: &[f32], timeline_samples: u64);
    /// Appends mixed samples to `out` and returns the number appended.
    fn drain_mixed(&mut self, out: &mut Vec<f32>) -> usize;
}

/// Default mixer implementation per design D-AudioMixer.
#[derive(Debug)]
pub struct DefaultAudioMixer {
    mic: TimelineTrack,
    system: TimelineTrack,
    next_emit_timeline: Option<u64>,
}

impl DefaultAudioMixer {
    pub fn new() -> Self {
        Self {
            mic: TimelineTrack::new(),
            system: TimelineTrack::new(),
            next_emit_timeline: None,
        }
    }

    #[cfg(test)]
    fn mic_retained_samples(&self) -> usize {
        self.mic.samples.len()
    }
}

impl Default for DefaultAudioMixer {
    fn default() -> Self {
        Self::new()
    }
}

impl AudioMixer for DefaultAudioMixer {
    fn push_mic(&mut self, samples: &[f32], timeline_samples: u64) {
        self.mic.push(samples, timeline_samples);
        self.trim_retention();
    }

    fn push_system(&mut self, samples: &[f32], timeline_samples: u64) {
        self.system.push(samples, timeline_samples);
        self.trim_retention();
    }

    fn drain_mixed(&mut self, out: &mut Vec<f32>) -> usize {
        self.align_tracks();
        let start = out.len();
        while let Some(sample) = self.emit_next_sample() {
            out.push(sample);
        }
        out.len() - start
    }
}

impl DefaultAudioMixer {
    fn trim_retention(&mut self) {
        self.mic.trim_to_max(MAX_RETENTION_SAMPLES);
        self.system.trim_to_max(MAX_RETENTION_SAMPLES);
    }

    fn align_tracks(&mut self) {
        let mic_start = self.mic.start_timeline();
        let sys_start = self.system.start_timeline();
        if mic_start.is_none() || sys_start.is_none() {
            return;
        }
        let mic_start = mic_start.unwrap();
        let sys_start = sys_start.unwrap();
        if mic_start < sys_start {
            let gap = sys_start - mic_start;
            if gap > ALIGNMENT_SAMPLES {
                self.mic.drop_samples((gap - ALIGNMENT_SAMPLES) as usize);
            }
        } else if sys_start < mic_start {
            let gap = mic_start - sys_start;
            if gap > ALIGNMENT_SAMPLES {
                self.system.drop_samples((gap - ALIGNMENT_SAMPLES) as usize);
            }
        }
    }

    fn emit_next_sample(&mut self) -> Option<f32> {
        let emit_at = match self.next_emit_timeline {
            Some(t) => t,
            None => {
                let start = earliest_mixable_timeline(&self.mic, &self.system)?;
                self.next_emit_timeline = Some(start);
                start
            }
        };

        if !self.can_emit_at(emit_at) {
            return None;
        }

        let mic_sample = self.mic.sample_at(emit_at);
        let sys_sample = self.system.sample_at(emit_at);

        let mic_gain = self.mic.rms.gain();
        let sys_gain = self.system.rms.gain();

        if let Some(sample) = mic_sample {
            self.mic.rms.push(sample);
        }
        if let Some(sample) = sys_sample {
            self.system.rms.push(sample);
        }

        let mic_scaled = mic_sample.unwrap_or(0.0) * mic_gain;
        let sys_scaled = sys_sample.unwrap_or(0.0) * sys_gain;
        let mixed = soft_limit(mic_scaled + sys_scaled);

        self.mic.consume_through(emit_at);
        self.system.consume_through(emit_at);
        self.next_emit_timeline = Some(emit_at + 1);

        Some(mixed)
    }

    fn can_emit_at(&self, timeline: u64) -> bool {
        let mic_has = self.mic.covers(timeline);
        let sys_has = self.system.covers(timeline);
        let mic_active = self.mic.start_timeline().is_some();
        let sys_active = self.system.start_timeline().is_some();

        if !mic_active || !sys_active {
            return mic_has || sys_has;
        }

        if mic_has && sys_has {
            return true;
        }

        let mic_end = self.mic.end_timeline();
        let sys_end = self.system.end_timeline();
        let leading_ahead = (mic_end > sys_end && mic_has && !sys_has)
            || (sys_end > mic_end && sys_has && !mic_has);
        if leading_ahead {
            let gap = mic_end.abs_diff(sys_end);
            return gap <= ALIGNMENT_SAMPLES;
        }

        false
    }
}

fn earliest_mixable_timeline(mic: &TimelineTrack, system: &TimelineTrack) -> Option<u64> {
    match (mic.start_timeline(), system.start_timeline()) {
        (Some(m), Some(s)) => Some(m.max(s)),
        (Some(m), None) => Some(m),
        (None, Some(s)) => Some(s),
        (None, None) => None,
    }
}

fn soft_limit(sample: f32) -> f32 {
    sample.clamp(-SOFT_LIMIT, SOFT_LIMIT)
}

#[derive(Debug)]
struct TimelineTrack {
    start_timeline: Option<u64>,
    samples: VecDeque<f32>,
    rms: RmsWindow,
}

impl TimelineTrack {
    fn new() -> Self {
        Self {
            start_timeline: None,
            samples: VecDeque::new(),
            rms: RmsWindow::new(RMS_WINDOW_SAMPLES),
        }
    }

    fn push(&mut self, samples: &[f32], timeline_samples: u64) {
        if samples.is_empty() {
            return;
        }
        if self.start_timeline.is_none() {
            self.start_timeline = Some(timeline_samples);
        }
        self.samples.extend(samples);
    }

    fn start_timeline(&self) -> Option<u64> {
        self.start_timeline
    }

    fn end_timeline(&self) -> u64 {
        self.start_timeline
            .map(|s| s + self.samples.len() as u64)
            .unwrap_or(0)
    }

    fn covers(&self, timeline: u64) -> bool {
        match self.start_timeline {
            Some(start) => timeline >= start && timeline < start + self.samples.len() as u64,
            None => false,
        }
    }

    fn sample_at(&self, timeline: u64) -> Option<f32> {
        match self.start_timeline {
            Some(start) if timeline >= start => {
                let idx = (timeline - start) as usize;
                self.samples.get(idx).copied()
            }
            _ => None,
        }
    }

    fn consume_through(&mut self, timeline: u64) {
        match self.start_timeline {
            Some(start) if timeline >= start => {
                let consumed = (timeline - start + 1) as usize;
                if consumed >= self.samples.len() {
                    self.samples.clear();
                    self.start_timeline = None;
                } else {
                    self.samples.drain(..consumed);
                    self.start_timeline = Some(start + consumed as u64);
                }
            }
            _ => {}
        }
    }

    fn drop_samples(&mut self, count: usize) {
        if count == 0 {
            return;
        }
        let drop = count.min(self.samples.len());
        self.samples.drain(..drop);
        if let Some(start) = self.start_timeline {
            self.start_timeline = Some(start + drop as u64);
        }
        if self.samples.is_empty() {
            self.start_timeline = None;
        }
    }

    fn trim_to_max(&mut self, max_samples: usize) {
        if self.samples.len() <= max_samples {
            return;
        }
        let excess = self.samples.len() - max_samples;
        self.drop_samples(excess);
    }
}

#[derive(Debug)]
struct RmsWindow {
    buf: VecDeque<f32>,
    capacity: usize,
    sum_sq: f64,
}

impl RmsWindow {
    fn new(capacity: usize) -> Self {
        Self {
            buf: VecDeque::with_capacity(capacity),
            capacity,
            sum_sq: 0.0,
        }
    }

    fn push(&mut self, sample: f32) {
        if self.buf.len() == self.capacity
            && let Some(old) = self.buf.pop_front()
        {
            self.sum_sq -= (old as f64) * (old as f64);
        }
        self.buf.push_back(sample);
        self.sum_sq += (sample as f64) * (sample as f64);
    }

    fn current_rms(&self) -> f32 {
        if self.buf.is_empty() {
            return MIN_RMS;
        }
        let rms = (self.sum_sq / self.buf.len() as f64).sqrt() as f32;
        rms.max(MIN_RMS)
    }

    fn gain(&self) -> f32 {
        TARGET_RMS / self.current_rms()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fill_sine(out: &mut Vec<f32>, frames: usize, amplitude: f32) {
        for i in 0..frames {
            let t = i as f32 / SAMPLE_RATE_HZ as f32;
            out.push(amplitude * (2.0 * std::f32::consts::PI * 440.0 * t).sin());
        }
    }

    #[test]
    // Testing Strategy 1: 片系統のみ入力 — クラッシュせず無音扱いにしない
    fn single_mic_stream_does_not_crash() {
        let mut mixer = DefaultAudioMixer::new();
        let mut mic = Vec::new();
        fill_sine(&mut mic, 4_800, 0.5);

        mixer.push_mic(&mic, 0);
        let mut out = Vec::new();
        let count = mixer.drain_mixed(&mut out);

        assert!(count > 0, "expected mixed output from mic-only input");
        assert!(out.iter().any(|s| s.abs() > 0.0));
        assert!(
            !out.iter().all(|&s| s.abs() < 1e-9),
            "mic-only output must not be treated as silence"
        );
    }

    #[test]
    // Testing Strategy 1: 片系統のみ入力 — クラッシュせず無音扱いにしない
    fn single_system_stream_does_not_crash() {
        let mut mixer = DefaultAudioMixer::new();
        let mut sys = Vec::new();
        fill_sine(&mut sys, 4_800, 0.3);

        mixer.push_system(&sys, 0);
        let mut out = Vec::new();
        let count = mixer.drain_mixed(&mut out);

        assert!(count > 0);
        assert!(out.iter().any(|s| s.abs() > 0.0));
        assert!(
            !out.iter().all(|&s| s.abs() < 1e-9),
            "system-only output must not be treated as silence"
        );
    }

    #[test]
    // Testing Strategy 1: 整列待ち — チャンク入力でも無音扱いにしない
    fn strategy_1_chunked_single_stream_not_silent_during_alignment_wait() {
        let mut mixer = DefaultAudioMixer::new();
        let mut out = Vec::new();

        for chunk in 0..6_u64 {
            let mut mic = Vec::new();
            fill_sine(&mut mic, ALIGNMENT_SAMPLES as usize, 0.5);
            mixer.push_mic(&mic, chunk * ALIGNMENT_SAMPLES);
            mixer.drain_mixed(&mut out);
        }

        assert!(
            !out.is_empty(),
            "chunked mic-only pushes must produce mixed output"
        );
        assert!(
            out.iter().any(|s| s.abs() > 0.01),
            "chunked mic-only input must not be treated as silence during alignment wait"
        );
    }

    #[test]
    // Testing Strategy 2: 大音量 mic + 小音量 system — ミックス後クリップしない (req 2.4)
    fn loud_mic_and_quiet_system_mix_without_clipping() {
        let mut mixer = DefaultAudioMixer::new();
        let frames = RMS_WINDOW_SAMPLES * 2;
        let mut loud_mic = Vec::new();
        let mut quiet_sys = Vec::new();
        fill_sine(&mut loud_mic, frames, 0.9);
        fill_sine(&mut quiet_sys, frames, 0.005);

        mixer.push_mic(&loud_mic, 0);
        mixer.push_system(&quiet_sys, 0);

        let mut out = Vec::new();
        let count = mixer.drain_mixed(&mut out);
        assert!(
            count > RMS_WINDOW_SAMPLES,
            "expected enough frames for RMS window"
        );

        let max_abs = out.iter().map(|s| s.abs()).fold(0.0_f32, f32::max);
        assert!(
            max_abs <= SOFT_LIMIT + 1e-6,
            "mixed output clipped above soft limit: max={max_abs}"
        );

        let rms = (out.iter().map(|s| (*s as f64) * (*s as f64)).sum::<f64>() / out.len() as f64)
            .sqrt() as f32;
        assert!(
            rms > 0.01,
            "quiet system should contribute after normalization, rms={rms}"
        );
    }

    #[test]
    fn retention_does_not_exceed_thirty_seconds() {
        let mut mixer = DefaultAudioMixer::new();
        let huge = vec![0.1_f32; MAX_RETENTION_SAMPLES + 1_000];
        mixer.push_mic(&huge, 0);

        assert!(
            mixer.mic_retained_samples() <= MAX_RETENTION_SAMPLES,
            "mic buffer exceeded 30 s retention cap"
        );
    }

    #[test]
    fn soft_limiter_clamps_extreme_sum() {
        assert_eq!(soft_limit(1.5), SOFT_LIMIT);
        assert_eq!(soft_limit(-2.0), -SOFT_LIMIT);
        assert!((soft_limit(0.5) - 0.5).abs() < 1e-6);
    }
}
