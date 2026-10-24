//! Inter-listener calibration: cross-correlates two recordings of the same
//! playback to measure the timing offset between two listeners' outputs.
//!
//! The correlation runs on amplitude envelopes (mean absolute sample per
//! block) rather than raw audio: it is cheap, robust to phase differences
//! between devices, and precise to a few milliseconds — well inside the
//! PRD's 20–50 ms synchronization target.

/// Converts interleaved PCM16 samples to mono f32 in -1.0..=1.0.
pub fn pcm16_to_mono(samples: &[i16], channels: usize) -> Vec<f32> {
    if channels <= 1 {
        return samples.iter().map(|&s| f32::from(s) / 32_768.0).collect();
    }
    samples
        .chunks(channels)
        .map(|frame| {
            frame.iter().map(|&s| f32::from(s) / 32_768.0).sum::<f32>() / frame.len() as f32
        })
        .collect()
}

/// Mean absolute amplitude per `block` samples.
pub fn envelope(signal: &[f32], block: usize) -> Vec<f32> {
    if block == 0 {
        return signal.to_vec();
    }
    signal
        .chunks(block)
        .map(|chunk| chunk.iter().map(|s| s.abs()).sum::<f32>() / chunk.len() as f32)
        .collect()
}

/// Normalized cross-correlation peak between `a` and `b`.
///
/// A positive lag means `b` trails `a` (the second listener heard the same
/// content later). `quality` is the peak correlation in -1.0..=1.0; values
/// below ~0.2 mean the recordings do not match usefully.
pub fn best_lag(a: &[f32], b: &[f32], max_lag: usize) -> (isize, f64) {
    let mut best = (0_isize, f64::MIN);
    for lag in -(max_lag as isize)..=(max_lag as isize) {
        let mut cross = 0.0_f64;
        let mut energy_a = 0.0_f64;
        let mut energy_b = 0.0_f64;
        let mut count = 0.0_f64;
        for (index, a_value) in a.iter().enumerate() {
            let b_index = index as isize + lag;
            if b_index < 0 || b_index as usize >= b.len() {
                continue;
            }
            let sample_a = f64::from(*a_value);
            let sample_b = f64::from(b[b_index as usize]);
            cross += sample_a * sample_b;
            energy_a += sample_a * sample_a;
            energy_b += sample_b * sample_b;
            count += 1.0;
        }
        if count < 8.0 {
            continue;
        }
        let denominator = (energy_a * energy_b).sqrt();
        if denominator <= f64::EPSILON {
            continue;
        }
        let score = cross / denominator;
        if score > best.1 {
            best = (lag, score);
        }
    }
    if best.1 == f64::MIN { (0, 0.0) } else { best }
}

/// A parsed PCM WAV file: mono f32 samples and their sample rate.
#[derive(Clone, Debug, PartialEq)]
pub struct WavAudio {
    pub samples: Vec<f32>,
    pub sample_rate: u32,
}

/// Reads a RIFF/WAV file with 16-bit PCM data into mono samples.
pub fn read_wav_mono(bytes: &[u8]) -> Result<WavAudio, String> {
    if bytes.len() < 12 || &bytes[0..4] != b"RIFF" || &bytes[8..12] != b"WAVE" {
        return Err("not a RIFF/WAVE file".to_owned());
    }

    let mut format: Option<(u16, u16, u32)> = None; // (channels, bits, sample_rate)
    let mut data: Option<Vec<f32>> = None;
    let mut cursor = 12;
    while cursor + 8 <= bytes.len() {
        let chunk_id = &bytes[cursor..cursor + 4];
        let chunk_len = u32::from_le_bytes(
            bytes[cursor + 4..cursor + 8]
                .try_into()
                .expect("chunk length"),
        ) as usize;
        let body_start = cursor + 8;
        let body_end = body_start.saturating_add(chunk_len).min(bytes.len());

        if chunk_id == b"fmt " && body_end - body_start >= 16 {
            let audio_format =
                u16::from_le_bytes(bytes[body_start..body_start + 2].try_into().expect("fmt"));
            let channels = u16::from_le_bytes(
                bytes[body_start + 2..body_start + 4]
                    .try_into()
                    .expect("fmt"),
            );
            let sample_rate = u32::from_le_bytes(
                bytes[body_start + 4..body_start + 8]
                    .try_into()
                    .expect("fmt"),
            );
            let bits = u16::from_le_bytes(
                bytes[body_start + 14..body_start + 16]
                    .try_into()
                    .expect("fmt"),
            );
            if audio_format != 1 {
                return Err(format!("unsupported WAV encoding {audio_format}; need PCM"));
            }
            if bits != 16 {
                return Err(format!("unsupported WAV bit depth {bits}; need 16-bit"));
            }
            format = Some((channels, bits, sample_rate));
        } else if chunk_id == b"data" {
            let body = &bytes[body_start..body_end];
            let usable = body.len() - body.len() % 2;
            let samples: Vec<i16> = body[..usable]
                .as_chunks::<2>()
                .0
                .iter()
                .map(|pair| i16::from_le_bytes([pair[0], pair[1]]))
                .collect();
            let channels = usize::from(format.map(|f| f.0).unwrap_or(1));
            data = Some(pcm16_to_mono(&samples, channels));
        }

        cursor = body_start + chunk_len + (chunk_len % 2);
    }

    let Some((channels, _, sample_rate)) = format else {
        return Err("WAV file has no format chunk".to_owned());
    };
    if channels == 0 {
        return Err("WAV file declares zero channels".to_owned());
    }
    let Some(samples) = data else {
        return Err("WAV file has no data chunk".to_owned());
    };
    Ok(WavAudio {
        samples,
        sample_rate,
    })
}

/// Measures the offset between two recordings of the same playback.
///
/// Returns `(lag_seconds, quality)`; a positive lag means `b` heard the
/// content later than `a`.
pub fn measure_offset(a: &WavAudio, b: &WavAudio, max_lag_seconds: f64) -> (f64, f64) {
    // A common envelope rate keeps the lag scale comparable even when the
    // two devices recorded at different sample rates.
    const ENVELOPE_HZ: f64 = 750.0;
    let block_a = ((f64::from(a.sample_rate) / ENVELOPE_HZ).round() as usize).max(1);
    let block_b = ((f64::from(b.sample_rate) / ENVELOPE_HZ).round() as usize).max(1);
    let envelope_a = envelope(&a.samples, block_a);
    let envelope_b = envelope(&b.samples, block_b);
    let max_lag = ((max_lag_seconds * ENVELOPE_HZ).round() as usize).max(1);

    let (lag, quality) = best_lag(&envelope_a, &envelope_b, max_lag);
    (lag as f64 / ENVELOPE_HZ, quality)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Percussive envelope: bursts of decaying noise at a steady beat.
    fn burst_train(beats: usize, envelope_hz: f64, period_s: f64) -> Vec<f32> {
        let mut signal = vec![0.0_f32; (beats as f64 * period_s * envelope_hz) as usize + 16];
        let period = (period_s * envelope_hz) as usize;
        for beat in 0..beats {
            let start = beat * period;
            for offset in 0..8.min(signal.len() - start) {
                signal[start + offset] = ((8 - offset) as f32) / 8.0;
            }
        }
        signal
    }

    #[test]
    fn best_lag_recovers_a_known_shift() {
        let signal = burst_train(30, 100.0, 0.5_f64);
        let shifted = shift(&signal, 12);
        let (lag, quality) = best_lag(&signal, &shifted, 200);
        assert_eq!(lag, 12);
        assert!(quality > 0.5, "quality was {quality}");
    }

    fn shift(signal: &[f32], lag: usize) -> Vec<f32> {
        let mut shifted = vec![0.0; signal.len() + lag];
        shifted[lag..].copy_from_slice(signal);
        shifted
    }

    #[test]
    fn measure_offset_reports_seconds_between_recordings() {
        // Build both recordings from the same 750 Hz envelope domain so the
        // only difference is a 45-block (60 ms) delay on listener B.
        let envelope_hz = 750.0_f64;
        let a_envelope = burst_train(60, envelope_hz, 0.4);
        let lag_blocks = 45_usize; // 45 / 750 Hz = 60 ms
        let mut b_envelope = vec![0.0_f32; a_envelope.len() + lag_blocks];
        b_envelope[lag_blocks..].copy_from_slice(&a_envelope);

        let to_wav = |envelope: &[f32]| WavAudio {
            samples: envelope
                .iter()
                .flat_map(|&value| [value * 0.5_f32; 64])
                .collect(),
            sample_rate: 48_000,
        };
        let a = to_wav(&a_envelope);
        let b = to_wav(&b_envelope);

        let (lag, quality) = measure_offset(&a, &b, 0.75);
        assert!(quality > 0.3, "quality was {quality}");
        assert!(
            (lag - 0.060).abs() <= 0.010,
            "lag was {lag:.4}s, expected ~0.060s"
        );
    }

    #[test]
    fn wav_parser_reads_a_minimal_pcm16_file() {
        // 1 channel, 8 samples, 16-bit.
        let mut wav = Vec::new();
        wav.extend_from_slice(b"RIFF");
        let data: Vec<i16> = (0..8).map(|i| i * 100).collect();
        let data_bytes: Vec<u8> = data.iter().flat_map(|s| s.to_le_bytes()).collect();
        let riff_len = 4 + (8 + 16) + (8 + data_bytes.len() as u32);
        wav.extend_from_slice(&riff_len.to_le_bytes());
        wav.extend_from_slice(b"WAVE");
        wav.extend_from_slice(b"fmt ");
        wav.extend_from_slice(&16_u32.to_le_bytes());
        wav.extend_from_slice(&1_u16.to_le_bytes()); // PCM
        wav.extend_from_slice(&1_u16.to_le_bytes()); // mono
        wav.extend_from_slice(&48_000_u32.to_le_bytes());
        wav.extend_from_slice(&96_000_u32.to_le_bytes()); // byte rate
        wav.extend_from_slice(&2_u16.to_le_bytes()); // block align
        wav.extend_from_slice(&16_u16.to_le_bytes()); // bits
        wav.extend_from_slice(b"data");
        wav.extend_from_slice(&(data_bytes.len() as u32).to_le_bytes());
        wav.extend_from_slice(&data_bytes);

        let parsed = read_wav_mono(&wav).expect("minimal WAV should parse");
        assert_eq!(parsed.sample_rate, 48_000);
        assert_eq!(parsed.samples.len(), 8);
        assert!((parsed.samples[7] - f32::from(700_i16) / 32_768.0).abs() < 1e-6);
    }

    #[test]
    fn wav_parser_rejects_non_wav_and_wrong_encodings() {
        assert!(read_wav_mono(b"not a wav file").is_err());
    }
}
