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
