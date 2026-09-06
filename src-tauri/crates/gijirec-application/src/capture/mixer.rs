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

/// Maximum normalization gain (+12 dB).
///
/// 無制限に `TARGET_RMS / rms` を掛けると、無音側トラックの床ノイズ（RMS 0.001 前後）が
/// 100 倍以上に増幅されて -20 dBFS に張り付き、発話側と同レベルのノイズとして混ざる。
/// 静かな発話（RMS 0.02〜0.05）を目標に寄せるには +12 dB で十分。
const MAX_GAIN: f32 = 4.0;

/// Noise gate: tracks whose 200 ms RMS is at or below this level are passed through
/// unamplified (gain 1.0). Matches the transcribe-side `SILENCE_RMS_THRESHOLD` (-42 dBFS)
/// so that a silent track never gets boosted above the VAD threshold by the mixer.
const NOISE_GATE_RMS: f32 = 0.008;

/// Largest forward timeline gap (1 s at 16 kHz) that is zero-filled when a track
/// resumes after a delivery pause (e.g. WASAPI loopback delivering nothing while
/// no application renders audio). Larger gaps restart the track at the new position.
const MAX_GAP_FILL_SAMPLES: u64 = 16_000;

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
        let timeline = live_timeline(&self.mic, self.next_emit_timeline, timeline_samples);
        self.mic.push(samples, timeline);
        self.trim_retention();
    }

    fn push_system(&mut self, samples: &[f32], timeline_samples: u64) {
        let timeline = live_timeline(&self.system, self.next_emit_timeline, timeline_samples);
        self.system.push(samples, timeline);
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

/// Resolves the timeline label for content arriving on `track`.
///
/// 呼び出し側のラベルは各ストリームの「配信済みサンプル数の累積」であり、配信開始が
/// 遅れたソース（ループバック初期化遅延、無再生時にパケットを出さない WASAPI ループバック）
/// は相手より恒常的に遅れたラベルを持つ。トラックが空のときに届いた内容は「いま」の音で
/// あり、既に出力した位置（emit cursor）より前へ置くことはできないため、カーソルへ
/// 引き上げる。引き上げずに整列処理へ渡すと、50 ms の整列窓を超えた遅れとして
/// 到着のたびに全量捨てられ、そのソースが一切ミックスされなくなる。
/// トラックが空でない場合はラベルを据え置き、`TimelineTrack::push` の連結規則に従う。
fn live_timeline(track: &TimelineTrack, emit_cursor: Option<u64>, label: u64) -> u64 {
    match (track.start_timeline(), emit_cursor) {
        (None, Some(cursor)) => label.max(cursor),
        _ => label,
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
        } else {
            self.fill_forward_gap(timeline_samples);
        }
        self.samples.extend(samples);
    }

    /// Handles a label ahead of the current end: zero-fills short delivery gaps,
    /// restarts the track for long ones. Labels at or behind the end append contiguously.
    fn fill_forward_gap(&mut self, timeline_samples: u64) {
        let gap = timeline_samples.saturating_sub(self.end_timeline());
        if gap == 0 {
            return;
        }
        if gap <= MAX_GAP_FILL_SAMPLES {
            self.samples
                .extend(std::iter::repeat_n(0.0_f32, gap as usize));
        } else {
            self.samples.clear();
            self.start_timeline = Some(timeline_samples);
        }
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
        gain_for_rms(self.current_rms())
    }
}

/// Normalization gain toward `TARGET_RMS`, bounded by `MAX_GAIN` and gated below
/// `NOISE_GATE_RMS`. The gate ramps linearly over `[NOISE_GATE_RMS, 2 * NOISE_GATE_RMS]`
/// to avoid pumping at the threshold.
fn gain_for_rms(rms: f32) -> f32 {
    if rms <= NOISE_GATE_RMS {
        return 1.0;
    }
    let normalizing = (TARGET_RMS / rms.max(MIN_RMS)).min(MAX_GAIN);
    let blend = ((rms - NOISE_GATE_RMS) / NOISE_GATE_RMS).min(1.0);
    1.0 + (normalizing - 1.0) * blend
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

    fn rms_of(samples: &[f32]) -> f32 {
        (samples
            .iter()
            .map(|s| (*s as f64) * (*s as f64))
            .sum::<f64>()
            / samples.len().max(1) as f64)
            .sqrt() as f32
    }

    /// Deterministic pseudo-noise at a given peak amplitude (LCG, no external crate).
    fn fill_noise(out: &mut Vec<f32>, frames: usize, amplitude: f32) {
        let mut state: u32 = 0x1234_5678;
        for _ in 0..frames {
            state = state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            let unit = (state >> 8) as f32 / (1u32 << 24) as f32; // [0, 1)
            out.push(amplitude * (unit * 2.0 - 1.0));
        }
    }

    #[test]
    // 回帰: 無音側トラックの床ノイズを -20 dBFS まで持ち上げない（ノイズゲート）
    fn silent_mic_noise_floor_is_not_boosted_while_system_speaks() {
        let mut mixer = DefaultAudioMixer::new();
        let frames = RMS_WINDOW_SAMPLES * 4;
        let mut mic_noise = Vec::new();
        let mut sys_speech = Vec::new();
        fill_noise(&mut mic_noise, frames, 0.003); // 床ノイズ RMS ≈ 0.0017
        fill_sine(&mut sys_speech, frames, 0.3);

        mixer.push_mic(&mic_noise, 0);
        mixer.push_system(&sys_speech, 0);
        let mut out = Vec::new();
        mixer.drain_mixed(&mut out);

        // RMS 窓が埋まった後半だけを評価する。
        let tail = &out[out.len() / 2..];
        let sys_tail = &sys_speech[sys_speech.len() / 2..];
        let sys_rms = rms_of(sys_tail);
        // system は -20 dBFS 付近へ正規化されている前提で、その理想ゲインを逆算する。
        let sys_gain = TARGET_RMS / sys_rms;
        let residual: Vec<f32> = tail
            .iter()
            .zip(sys_tail)
            .map(|(mixed, sys)| mixed - sys * sys_gain)
            .collect();
        let residual_rms = rms_of(&residual);
        assert!(
            residual_rms < NOISE_GATE_RMS,
            "mic noise floor leaked into the mix: residual_rms={residual_rms}"
        );
    }

    #[test]
    fn mic_only_noise_floor_passes_through_unamplified() {
        let mut mixer = DefaultAudioMixer::new();
        let frames = RMS_WINDOW_SAMPLES * 4;
        let mut mic_noise = Vec::new();
        fill_noise(&mut mic_noise, frames, 0.003);

        mixer.push_mic(&mic_noise, 0);
        let mut out = Vec::new();
        mixer.drain_mixed(&mut out);

        let in_rms = rms_of(&mic_noise);
        let out_rms = rms_of(&out);
        assert!(
            (out_rms - in_rms).abs() < in_rms * 0.05,
            "gated track must pass through at unity gain: in={in_rms} out={out_rms}"
        );
    }

    #[test]
    fn gain_is_bounded_and_gated() {
        assert_eq!(gain_for_rms(0.0), 1.0);
        assert_eq!(gain_for_rms(NOISE_GATE_RMS), 1.0);
        assert!(gain_for_rms(NOISE_GATE_RMS * 1.5) < MAX_GAIN);
        assert!((gain_for_rms(NOISE_GATE_RMS * 2.0) - MAX_GAIN).abs() < 1e-6);
        assert!((gain_for_rms(0.05) - 2.0).abs() < 1e-6);
        // 大音量は減衰させる（クリップ回避、req 2.4）
        assert!(gain_for_rms(0.6) < 0.2);
        for rms in [1e-6_f32, 0.001, 0.01, 0.03, 0.1, 0.5] {
            assert!(gain_for_rms(rms) <= MAX_GAIN, "rms={rms}");
        }
    }

    #[test]
    // 回帰: 配信が途切れた後にタイムラインが進んで再開しても、旧サンプルへ連結せず整列する
    fn track_resuming_after_delivery_gap_stays_aligned() {
        let mut mixer = DefaultAudioMixer::new();
        let mut out = Vec::new();

        // system: 先頭 800 サンプルの後、7_200 サンプル分（450 ms）配信が途切れて再開
        let mut sys_head = Vec::new();
        fill_sine(&mut sys_head, 800, 0.3);
        mixer.push_system(&sys_head, 0);
        let mut sys_tail = Vec::new();
        fill_sine(&mut sys_tail, 4_800, 0.3);
        mixer.push_system(&sys_tail, 8_000);

        // mic: 連続無音（ゲート内の床ノイズ）
        let mut mic = Vec::new();
        fill_noise(&mut mic, 12_800, 0.001);
        mixer.push_mic(&mic, 0);

        mixer.drain_mixed(&mut out);
        assert_eq!(
            out.len(),
            12_800,
            "mixed output must cover the full mic span"
        );

        let gap = &out[800..8_000];
        assert!(
            rms_of(gap) < NOISE_GATE_RMS,
            "gap must be near-silent, rms={}",
            rms_of(gap)
        );
        let resumed = &out[8_000..12_800];
        assert!(
            rms_of(resumed) > 0.05,
            "resumed system audio must be present at its timeline, rms={}",
            rms_of(resumed)
        );
    }

    #[test]
    // 回帰: 配信開始が 50 ms 以上遅れたソースが、処理ループの定常状態（5 ms ごとに
    // push→全量 drain）で以降ずっと捨てられない
    fn late_starting_system_stream_is_mixed_in_steady_state() {
        let mut mixer = DefaultAudioMixer::new();
        let mut out = Vec::new();
        let batch = 80_usize; // 5 ms @ 16 kHz
        let mut mic_timeline = 0_u64;
        let mut sys_timeline = 0_u64;
        let mic_silence = vec![0.0_f32; batch];
        let mut sys_batch = Vec::new();
        fill_sine(&mut sys_batch, batch, 0.3);

        // 先頭 300 ms は mic のみ配信（system はまだパケットが来ない）
        for _ in 0..60 {
            mixer.push_mic(&mic_silence, mic_timeline);
            mic_timeline += batch as u64;
            mixer.drain_mixed(&mut out);
        }
        let mic_only_len = out.len();

        // 以降 1 s: 両方配信。system のラベルは累積カウンタなので 300 ms 遅れたまま
        for _ in 0..200 {
            mixer.push_mic(&mic_silence, mic_timeline);
            mic_timeline += batch as u64;
            mixer.push_system(&sys_batch, sys_timeline);
            sys_timeline += batch as u64;
            mixer.drain_mixed(&mut out);
        }

        assert!(
            out.len() >= mic_only_len + 190 * batch,
            "mixed output must keep flowing, got {} samples",
            out.len()
        );
        // RMS 窓のランプイン（200 ms）を除いた後半を評価する
        let tail = &out[mic_only_len + RMS_WINDOW_SAMPLES..];
        assert!(
            rms_of(tail) > 0.05,
            "late-starting system audio must be present, rms={}",
            rms_of(tail)
        );
    }

    #[test]
    fn track_gap_beyond_fill_limit_restarts_at_new_timeline() {
        let mut track = TimelineTrack::new();
        track.push(&[0.1; 100], 0);
        track.push(&[0.2; 50], 100 + MAX_GAP_FILL_SAMPLES + 1);
        assert_eq!(track.start_timeline(), Some(100 + MAX_GAP_FILL_SAMPLES + 1));
        assert_eq!(track.samples.len(), 50);
    }
}
