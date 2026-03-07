use anyhow::Result;
use candle_core::{Device, Tensor};
use rand::Rng;

/// Sampling strategy for token generation.
pub enum Sampling {
    ArgMax,
    All { temperature: f64 },
    TopK { k: usize, temperature: f64 },
    TopP { p: f64, temperature: f64 },
    TopKThenTopP { k: usize, p: f64, temperature: f64 },
}

impl Sampling {
    /// Create the appropriate sampling strategy from parameters.
    pub fn from_params(temperature: f64, top_p: Option<f64>, top_k: Option<usize>) -> Self {
        if temperature <= 0.0 {
            return Self::ArgMax;
        }
        match (top_k, top_p) {
            (Some(k), Some(p)) => Self::TopKThenTopP {
                k,
                p,
                temperature,
            },
            (Some(k), None) => Self::TopK {
                k,
                temperature,
            },
            (None, Some(p)) => Self::TopP { p, temperature },
            (None, None) => Self::All { temperature },
        }
    }

    /// Sample a token index from logits.
    pub fn sample(&self, logits: &Tensor) -> Result<u32> {
        let logits = logits.to_dtype(candle_core::DType::F32)?.to_device(&Device::Cpu)?;
        let logits_vec: Vec<f32> = logits.to_vec1()?;

        let next_token = match self {
            Self::ArgMax => {
                logits_vec
                    .iter()
                    .enumerate()
                    .max_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
                    .map(|(i, _)| i as u32)
                    .unwrap_or(0)
            }
            Self::All { temperature } => {
                let scaled = apply_temperature(&logits_vec, *temperature);
                sample_multinomial(&scaled)?
            }
            Self::TopK { k, temperature } => {
                let scaled = apply_temperature(&logits_vec, *temperature);
                let filtered = top_k_filter(&scaled, *k);
                sample_multinomial(&filtered)?
            }
            Self::TopP { p, temperature } => {
                let scaled = apply_temperature(&logits_vec, *temperature);
                let filtered = top_p_filter(&scaled, *p);
                sample_multinomial(&filtered)?
            }
            Self::TopKThenTopP { k, p, temperature } => {
                let scaled = apply_temperature(&logits_vec, *temperature);
                let filtered = top_k_filter(&scaled, *k);
                let filtered = top_p_filter(&filtered, *p);
                sample_multinomial(&filtered)?
            }
        };

        Ok(next_token)
    }
}

/// Apply repeat penalty to logits for previously generated tokens.
pub fn apply_repeat_penalty(logits: &Tensor, penalty: f32, context: &[u32]) -> Result<Tensor> {
    if penalty == 1.0 || context.is_empty() {
        return Ok(logits.clone());
    }

    let device = logits.device();
    let mut logits_vec: Vec<f32> = logits
        .to_dtype(candle_core::DType::F32)?
        .to_device(&Device::Cpu)?
        .to_vec1()?;

    for &token_id in context {
        let idx = token_id as usize;
        if idx < logits_vec.len() {
            if logits_vec[idx] > 0.0 {
                logits_vec[idx] /= penalty;
            } else {
                logits_vec[idx] *= penalty;
            }
        }
    }

    Ok(Tensor::from_vec(logits_vec, logits.shape(), device)?)
}

// ── Internal helpers ──────────────────────────────────────────────────────

fn apply_temperature(logits: &[f32], temperature: f64) -> Vec<f32> {
    let temp = temperature as f32;
    logits.iter().map(|&l| l / temp).collect()
}

fn softmax(logits: &[f32]) -> Vec<f32> {
    let max = logits.iter().cloned().fold(f32::NEG_INFINITY, f32::max);
    let exps: Vec<f32> = logits.iter().map(|&l| (l - max).exp()).collect();
    let sum: f32 = exps.iter().sum();
    let safe_sum = sum.max(f32::EPSILON);
    exps.iter().map(|&e| e / safe_sum).collect()
}

fn top_k_filter(logits: &[f32], k: usize) -> Vec<f32> {
    let mut indexed: Vec<(usize, f32)> = logits.iter().copied().enumerate().collect();
    indexed.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

    let mut result = vec![f32::NEG_INFINITY; logits.len()];
    for (i, (idx, val)) in indexed.into_iter().enumerate() {
        if i >= k {
            break;
        }
        result[idx] = val;
    }
    result
}

fn top_p_filter(logits: &[f32], p: f64) -> Vec<f32> {
    let probs = softmax(logits);
    let mut indexed: Vec<(usize, f32)> = probs.iter().copied().enumerate().collect();
    indexed.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

    let mut cumulative = 0.0f32;
    let mut result = vec![f32::NEG_INFINITY; logits.len()];

    for (idx, prob) in indexed {
        if cumulative >= p as f32 && cumulative > 0.0 {
            break;
        }
        result[idx] = logits[idx];
        cumulative += prob;
    }

    result
}

fn sample_multinomial(logits: &[f32]) -> Result<u32> {
    let probs = softmax(logits);

    let random: f32 = rand::rng().random::<f32>();

    let mut cumulative = 0.0f32;
    for (i, &prob) in probs.iter().enumerate() {
        cumulative += prob;
        if cumulative >= random {
            return Ok(i as u32);
        }
    }

    // Fallback to last token
    Ok((probs.len() - 1) as u32)
}

// ── Tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── softmax ──────────────────────────────────────────────────────────

    #[test]
    fn softmax_outputs_sum_to_one() {
        let logits = vec![1.0, 2.0, 3.0, 4.0];
        let probs = softmax(&logits);
        let sum: f32 = probs.iter().sum();
        assert!((sum - 1.0).abs() < 1e-5, "Softmax sum should be ~1.0, got {}", sum);
    }

    #[test]
    fn softmax_all_zeros() {
        let logits = vec![0.0, 0.0, 0.0];
        let probs = softmax(&logits);
        let sum: f32 = probs.iter().sum();
        assert!((sum - 1.0).abs() < 1e-5, "Softmax of all zeros should sum to ~1.0");
        // All probabilities should be equal
        for &p in &probs {
            assert!((p - 1.0 / 3.0).abs() < 1e-5);
        }
    }

    #[test]
    fn softmax_no_nan() {
        let logits = vec![0.0, 0.0, 0.0, 0.0];
        let probs = softmax(&logits);
        for &p in &probs {
            assert!(!p.is_nan(), "Softmax should not produce NaN");
            assert!(!p.is_infinite(), "Softmax should not produce Inf");
        }
    }

    #[test]
    fn softmax_extreme_positive() {
        let logits = vec![1000.0, 1000.0, 0.0];
        let probs = softmax(&logits);
        let sum: f32 = probs.iter().sum();
        assert!((sum - 1.0).abs() < 1e-5, "Softmax with extreme values should still sum to ~1.0");
        // The first two should dominate
        assert!(probs[0] > 0.4);
        assert!(probs[1] > 0.4);
        assert!(probs[2] < 1e-5);
    }

    #[test]
    fn softmax_extreme_negative() {
        let logits = vec![-1000.0, -1000.0, 0.0];
        let probs = softmax(&logits);
        let sum: f32 = probs.iter().sum();
        assert!((sum - 1.0).abs() < 1e-5);
        // The last element should dominate
        assert!(probs[2] > 0.99);
    }

    #[test]
    fn softmax_single_element() {
        let logits = vec![5.0];
        let probs = softmax(&logits);
        assert_eq!(probs.len(), 1);
        assert!((probs[0] - 1.0).abs() < 1e-5);
    }

    #[test]
    fn softmax_preserves_ordering() {
        let logits = vec![1.0, 3.0, 2.0];
        let probs = softmax(&logits);
        assert!(probs[1] > probs[2]);
        assert!(probs[2] > probs[0]);
    }

    // ── apply_temperature ────────────────────────────────────────────────

    #[test]
    fn apply_temperature_one_is_identity() {
        let logits = vec![1.0, 2.0, 3.0];
        let scaled = apply_temperature(&logits, 1.0);
        for (a, b) in logits.iter().zip(scaled.iter()) {
            assert!((a - b).abs() < 1e-6);
        }
    }

    #[test]
    fn apply_temperature_high_flattens_distribution() {
        let logits = vec![1.0, 5.0, 3.0];
        let scaled = apply_temperature(&logits, 100.0);
        // High temperature makes values closer together
        let range = scaled.iter().cloned().fold(f32::NEG_INFINITY, f32::max)
            - scaled.iter().cloned().fold(f32::INFINITY, f32::min);
        assert!(range < 0.1, "High temperature should flatten distribution, range={}", range);
    }

    #[test]
    fn apply_temperature_low_sharpens_distribution() {
        let logits = vec![1.0, 5.0, 3.0];
        let scaled = apply_temperature(&logits, 0.01);
        // Low temperature amplifies differences
        let range_original = 5.0 - 1.0;
        let range_scaled = scaled.iter().cloned().fold(f32::NEG_INFINITY, f32::max)
            - scaled.iter().cloned().fold(f32::INFINITY, f32::min);
        assert!(range_scaled > range_original * 10.0);
    }

    // ── Sampling::from_params ────────────────────────────────────────────

    #[test]
    fn from_params_zero_temperature_gives_argmax() {
        let sampling = Sampling::from_params(0.0, None, None);
        assert!(matches!(sampling, Sampling::ArgMax));
    }

    #[test]
    fn from_params_negative_temperature_gives_argmax() {
        let sampling = Sampling::from_params(-1.0, None, None);
        assert!(matches!(sampling, Sampling::ArgMax));
    }

    #[test]
    fn from_params_temperature_only() {
        let sampling = Sampling::from_params(0.7, None, None);
        assert!(matches!(sampling, Sampling::All { temperature } if (temperature - 0.7).abs() < 1e-6));
    }

    #[test]
    fn from_params_top_k_only() {
        let sampling = Sampling::from_params(0.7, None, Some(50));
        assert!(matches!(sampling, Sampling::TopK { k: 50, .. }));
    }

    #[test]
    fn from_params_top_p_only() {
        let sampling = Sampling::from_params(0.7, Some(0.9), None);
        assert!(matches!(sampling, Sampling::TopP { .. }));
    }

    #[test]
    fn from_params_top_k_and_top_p() {
        let sampling = Sampling::from_params(0.7, Some(0.9), Some(50));
        assert!(matches!(sampling, Sampling::TopKThenTopP { k: 50, .. }));
    }

    // ── top_k_filter ─────────────────────────────────────────────────────

    #[test]
    fn top_k_filter_keeps_k_highest() {
        let logits = vec![1.0, 5.0, 3.0, 2.0, 4.0];
        let filtered = top_k_filter(&logits, 2);
        // Should keep indices 1 (5.0) and 4 (4.0), rest should be -inf
        assert_eq!(filtered[1], 5.0);
        assert_eq!(filtered[4], 4.0);
        assert_eq!(filtered[0], f32::NEG_INFINITY);
        assert_eq!(filtered[2], f32::NEG_INFINITY);
        assert_eq!(filtered[3], f32::NEG_INFINITY);
    }

    #[test]
    fn top_k_filter_k_equals_length() {
        let logits = vec![1.0, 2.0, 3.0];
        let filtered = top_k_filter(&logits, 3);
        // All should be kept
        assert_eq!(filtered, logits);
    }

    #[test]
    fn top_k_filter_k_one() {
        let logits = vec![1.0, 5.0, 3.0];
        let filtered = top_k_filter(&logits, 1);
        assert_eq!(filtered[1], 5.0);
        assert_eq!(filtered[0], f32::NEG_INFINITY);
        assert_eq!(filtered[2], f32::NEG_INFINITY);
    }

    // ── top_p_filter ─────────────────────────────────────────────────────

    #[test]
    fn top_p_filter_keeps_dominant_token() {
        // One token has overwhelmingly high probability
        let logits = vec![-100.0, 100.0, -100.0];
        let filtered = top_p_filter(&logits, 0.9);
        // The dominant token should be kept
        assert!(filtered[1] > f32::NEG_INFINITY);
    }

    #[test]
    fn top_p_filter_one_keeps_all() {
        let logits = vec![1.0, 2.0, 3.0];
        let filtered = top_p_filter(&logits, 1.0);
        // p=1.0 should keep all tokens
        for &v in &filtered {
            assert!(v > f32::NEG_INFINITY);
        }
    }

    // ── ArgMax determinism ───────────────────────────────────────────────

    #[test]
    fn argmax_picks_highest() {
        let logits_vec = vec![1.0f32, 3.0, 2.0, 0.5];
        let max_idx = logits_vec
            .iter()
            .enumerate()
            .max_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
            .map(|(i, _)| i as u32)
            .unwrap_or(0);
        assert_eq!(max_idx, 1);
    }
}
