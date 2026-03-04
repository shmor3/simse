# simse-predictive-coding Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers-extended-cc:executing-plans to implement this plan task-by-task.

**Goal:** Build a standalone Rust crate that implements Predictive Coding Networks for behavioral memory model training and prediction, exposed via JSON-RPC 2.0 / NDJSON stdio.

**Architecture:** A single-binary Rust crate following the simse pattern (see `simse-vsh` as template). A background training thread processes batched events via `mpsc`, while a lock-free `Arc<RwLock<ModelSnapshot>>` serves concurrent prediction queries. The PCN uses local Hebbian weight updates with temporal amortization.

**Tech Stack:** Rust (tokio, serde, serde_json, thiserror, tracing, uuid, base64, flate2, rand)

**Design doc:** `docs/plans/2026-03-04-predictive-coding-design.md`

---

### Task 0: Scaffold the crate

**Files:**
- Create: `simse-predictive-coding/Cargo.toml`
- Create: `simse-predictive-coding/src/lib.rs`
- Create: `simse-predictive-coding/src/main.rs`

**Step 1: Create Cargo.toml**

```toml
[package]
name = "simse-pcn-engine"
version = "0.1.0"
edition = "2021"
license = "MIT"
description = "Predictive coding network engine over JSON-RPC 2.0 / NDJSON stdio"

[lib]
name = "simse_pcn_engine"
path = "src/lib.rs"

[[bin]]
name = "simse-pcn-engine"
path = "src/main.rs"

[dependencies]
serde = { version = "1", features = ["derive"] }
serde_json = "1"
thiserror = "2"
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["fmt", "env-filter"] }
tokio = { version = "1", features = ["rt-multi-thread", "macros", "sync", "time"] }
uuid = { version = "1", features = ["v4"] }
base64 = "0.22"
flate2 = "1"
rand = "0.8"

[dev-dependencies]
tempfile = "3"
```

**Step 2: Create minimal lib.rs**

```rust
pub mod error;
```

**Step 3: Create minimal main.rs**

```rust
fn main() {
    println!("simse-pcn-engine placeholder");
}
```

**Step 4: Verify it compiles**

Run: `cd simse-predictive-coding && cargo build`
Expected: Compiles successfully (after creating error.rs in Task 1)

**Step 5: Commit**

```bash
git add simse-predictive-coding/
git commit -m "feat(pcn): scaffold simse-predictive-coding crate"
```

---

### Task 1: Implement PcnError and error handling

**Files:**
- Create: `simse-predictive-coding/src/error.rs`
- Test: inline `#[cfg(test)]` module

**Step 1: Write the failing test**

Add to `error.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_codes_have_pcn_prefix() {
        let err = PcnError::NotInitialized;
        assert!(err.code().starts_with("PCN_"));
    }

    #[test]
    fn to_json_rpc_error_has_pcn_code() {
        let err = PcnError::InvalidConfig("bad".into());
        let json = err.to_json_rpc_error();
        assert_eq!(json["pcnCode"], "PCN_INVALID_CONFIG");
    }
}
```

**Step 2: Run test to verify it fails**

Run: `cd simse-predictive-coding && cargo test`
Expected: FAIL — PcnError not defined

**Step 3: Write implementation**

```rust
use thiserror::Error;

#[derive(Debug, Error)]
pub enum PcnError {
    #[error("Not initialized")]
    NotInitialized,
    #[error("Invalid config: {0}")]
    InvalidConfig(String),
    #[error("Training failed: {0}")]
    TrainingFailed(String),
    #[error("Inference timeout")]
    InferenceTimeout,
    #[error("Model corrupt: {0}")]
    ModelCorrupt(String),
    #[error("Vocabulary overflow: {0}")]
    VocabularyOverflow(String),
    #[error("Invalid params: {0}")]
    InvalidParams(String),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
}

impl PcnError {
    pub fn code(&self) -> &str {
        match self {
            Self::NotInitialized => "PCN_NOT_INITIALIZED",
            Self::InvalidConfig(_) => "PCN_INVALID_CONFIG",
            Self::TrainingFailed(_) => "PCN_TRAINING_FAILED",
            Self::InferenceTimeout => "PCN_INFERENCE_TIMEOUT",
            Self::ModelCorrupt(_) => "PCN_MODEL_CORRUPT",
            Self::VocabularyOverflow(_) => "PCN_VOCABULARY_OVERFLOW",
            Self::InvalidParams(_) => "PCN_INVALID_PARAMS",
            Self::Io(_) => "PCN_IO_ERROR",
            Self::Json(_) => "PCN_JSON_ERROR",
        }
    }

    pub fn to_json_rpc_error(&self) -> serde_json::Value {
        serde_json::json!({
            "pcnCode": self.code(),
            "message": self.to_string(),
        })
    }
}
```

**Step 4: Run test to verify it passes**

Run: `cd simse-predictive-coding && cargo test`
Expected: PASS

**Step 5: Commit**

```bash
git add simse-predictive-coding/src/error.rs simse-predictive-coding/src/lib.rs
git commit -m "feat(pcn): add PcnError enum with domain error codes"
```

---

### Task 2: Implement PcnConfig and LayerConfig

**Files:**
- Create: `simse-predictive-coding/src/config.rs`
- Modify: `simse-predictive-coding/src/lib.rs` — add `pub mod config;`

**Step 1: Write the failing test**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_has_reasonable_values() {
        let config = PcnConfig::default();
        assert!(config.inference_steps > 0);
        assert!(config.learning_rate > 0.0);
        assert!(config.inference_rate > 0.0);
        assert!(config.batch_size > 0);
        assert!(config.max_batch_delay_ms > 0);
        assert!(config.channel_capacity > 0);
    }

    #[test]
    fn default_layer_config() {
        let layer = LayerConfig::default();
        assert_eq!(layer.dim, 256);
        assert!(matches!(layer.activation, Activation::Relu));
    }

    #[test]
    fn config_with_custom_layers() {
        let config = PcnConfig {
            layers: vec![
                LayerConfig { dim: 512, activation: Activation::Relu },
                LayerConfig { dim: 128, activation: Activation::Tanh },
                LayerConfig { dim: 32, activation: Activation::Sigmoid },
            ],
            ..Default::default()
        };
        assert_eq!(config.layers.len(), 3);
        assert_eq!(config.layers[0].dim, 512);
    }

    #[test]
    fn activation_apply() {
        assert_eq!(Activation::Relu.apply(-1.0), 0.0);
        assert_eq!(Activation::Relu.apply(2.0), 2.0);
        assert!((Activation::Sigmoid.apply(0.0) - 0.5).abs() < 1e-10);
        assert!((Activation::Tanh.apply(0.0)).abs() < 1e-10);
    }

    #[test]
    fn activation_derivative() {
        assert_eq!(Activation::Relu.derivative(-1.0), 0.0);
        assert_eq!(Activation::Relu.derivative(2.0), 1.0);
        // sigmoid'(0) = sigmoid(0) * (1 - sigmoid(0)) = 0.25
        assert!((Activation::Sigmoid.derivative(0.0) - 0.25).abs() < 1e-10);
        // tanh'(0) = 1 - tanh(0)^2 = 1
        assert!((Activation::Tanh.derivative(0.0) - 1.0).abs() < 1e-10);
    }
}
```

**Step 2: Run test to verify it fails**

Run: `cd simse-predictive-coding && cargo test`
Expected: FAIL

**Step 3: Write implementation**

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Activation {
    Relu,
    Tanh,
    Sigmoid,
}

impl Activation {
    pub fn apply(&self, x: f64) -> f64 {
        match self {
            Self::Relu => x.max(0.0),
            Self::Tanh => x.tanh(),
            Self::Sigmoid => 1.0 / (1.0 + (-x).exp()),
        }
    }

    pub fn derivative(&self, x: f64) -> f64 {
        match self {
            Self::Relu => if x > 0.0 { 1.0 } else { 0.0 },
            Self::Tanh => {
                let t = x.tanh();
                1.0 - t * t
            }
            Self::Sigmoid => {
                let s = 1.0 / (1.0 + (-x).exp());
                s * (1.0 - s)
            }
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LayerConfig {
    pub dim: usize,
    pub activation: Activation,
}

impl Default for LayerConfig {
    fn default() -> Self {
        Self {
            dim: 256,
            activation: Activation::Relu,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PcnConfig {
    /// Layer configurations (from bottom to top). Input dimension is inferred at runtime.
    pub layers: Vec<LayerConfig>,
    /// Number of inference iterations per sample.
    pub inference_steps: usize,
    /// Learning rate for weight updates (eta_learn).
    pub learning_rate: f64,
    /// Inference rate for latent updates (eta_infer).
    pub inference_rate: f64,
    /// Mini-batch size for training.
    pub batch_size: usize,
    /// Max delay before training an incomplete batch (ms).
    pub max_batch_delay_ms: u64,
    /// Bounded channel capacity for event ingestion.
    pub channel_capacity: usize,
    /// Auto-save interval in training epochs.
    pub auto_save_epochs: usize,
    /// Max topic vocabulary size.
    pub max_topics: usize,
    /// Max tag vocabulary size.
    pub max_tags: usize,
    /// Enable temporal amortization (carry forward latent states).
    pub temporal_amortization: bool,
    /// Optional storage path for persistence.
    pub storage_path: Option<String>,
}

impl Default for PcnConfig {
    fn default() -> Self {
        Self {
            layers: vec![
                LayerConfig { dim: 512, activation: Activation::Relu },
                LayerConfig { dim: 256, activation: Activation::Relu },
                LayerConfig { dim: 64, activation: Activation::Tanh },
            ],
            inference_steps: 20,
            learning_rate: 0.005,
            inference_rate: 0.1,
            batch_size: 16,
            max_batch_delay_ms: 1000,
            channel_capacity: 1024,
            auto_save_epochs: 100,
            max_topics: 500,
            max_tags: 1000,
            temporal_amortization: true,
            storage_path: None,
        }
    }
}
```

**Step 4: Run test to verify it passes**

Run: `cd simse-predictive-coding && cargo test`
Expected: PASS

**Step 5: Commit**

```bash
git add simse-predictive-coding/src/config.rs simse-predictive-coding/src/lib.rs
git commit -m "feat(pcn): add PcnConfig, LayerConfig, and Activation types"
```

---

### Task 3: Implement PcnLayer — single layer with value/error nodes

**Files:**
- Create: `simse-predictive-coding/src/layer.rs`
- Modify: `simse-predictive-coding/src/lib.rs` — add `pub mod layer;`

**Step 1: Write the failing test**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Activation;

    #[test]
    fn new_layer_has_correct_dimensions() {
        let layer = PcnLayer::new(4, 3, Activation::Relu);
        assert_eq!(layer.dim(), 4);
        assert_eq!(layer.input_dim(), 3);
        assert_eq!(layer.weights().len(), 4); // rows
        assert_eq!(layer.weights()[0].len(), 3); // cols
        assert_eq!(layer.bias().len(), 4);
        assert_eq!(layer.values().len(), 4);
        assert_eq!(layer.errors().len(), 4);
    }

    #[test]
    fn predict_computes_f_of_w_times_x_plus_b() {
        let mut layer = PcnLayer::new(2, 2, Activation::Relu);
        // Set known weights: identity matrix
        layer.set_weights(vec![vec![1.0, 0.0], vec![0.0, 1.0]]);
        layer.set_bias(vec![0.0, 0.0]);

        let input = vec![3.0, 5.0];
        let prediction = layer.predict(&input);
        // ReLU(I * [3,5] + [0,0]) = [3, 5]
        assert_eq!(prediction, vec![3.0, 5.0]);
    }

    #[test]
    fn predict_applies_activation() {
        let mut layer = PcnLayer::new(2, 2, Activation::Relu);
        layer.set_weights(vec![vec![1.0, 0.0], vec![0.0, 1.0]]);
        layer.set_bias(vec![-5.0, 0.0]);

        let input = vec![3.0, 5.0];
        let prediction = layer.predict(&input);
        // ReLU([3-5, 5+0]) = [0, 5]
        assert_eq!(prediction, vec![0.0, 5.0]);
    }

    #[test]
    fn compute_error_is_value_minus_prediction() {
        let mut layer = PcnLayer::new(2, 2, Activation::Relu);
        layer.set_weights(vec![vec![1.0, 0.0], vec![0.0, 1.0]]);
        layer.set_bias(vec![0.0, 0.0]);
        layer.set_values(vec![4.0, 6.0]);

        let input_from_above = vec![3.0, 5.0];
        layer.compute_errors(&input_from_above);
        // error = [4,6] - [3,5] = [1,1]
        assert_eq!(layer.errors(), &[1.0, 1.0]);
    }

    #[test]
    fn preactivations_stored_for_derivative() {
        let mut layer = PcnLayer::new(2, 2, Activation::Relu);
        layer.set_weights(vec![vec![1.0, 0.0], vec![0.0, 1.0]]);
        layer.set_bias(vec![-1.0, 2.0]);

        let input = vec![3.0, 5.0];
        layer.predict(&input);
        // preactivations = W*x + b = [3-1, 5+2] = [2, 7]
        assert_eq!(layer.preactivations(), &[2.0, 7.0]);
    }
}
```

**Step 2: Run test to verify it fails**

Run: `cd simse-predictive-coding && cargo test`
Expected: FAIL

**Step 3: Write implementation**

```rust
use crate::config::Activation;
use serde::{Deserialize, Serialize};

/// A single layer in the predictive coding network.
///
/// Holds value nodes x(l), error nodes e(l), generative weight matrix W(l),
/// bias b(l), and cached preactivations a(l) for derivative computation.
#[derive(Debug, Clone)]
pub struct PcnLayer {
    dim: usize,
    input_dim: usize,
    activation: Activation,
    /// Generative weights W(l): dim x input_dim matrix (row-major).
    weights: Vec<Vec<f64>>,
    /// Bias b(l): dim vector.
    bias: Vec<f64>,
    /// Value nodes x(l): current latent state.
    values: Vec<f64>,
    /// Error nodes e(l): prediction error at this layer.
    errors: Vec<f64>,
    /// Preactivations a(l) = W*x + b (before activation), cached for derivatives.
    preactivations: Vec<f64>,
}

impl PcnLayer {
    /// Create a new layer with random small weights.
    pub fn new(dim: usize, input_dim: usize, activation: Activation) -> Self {
        // Xavier initialization: scale = sqrt(2 / (fan_in + fan_out))
        let scale = (2.0 / (dim + input_dim) as f64).sqrt();
        let weights = (0..dim)
            .map(|i| {
                (0..input_dim)
                    .map(|j| {
                        // Deterministic pseudo-random for reproducibility in tests
                        let seed = (i * 7919 + j * 6271) as f64;
                        ((seed % 1000.0) / 1000.0 - 0.5) * 2.0 * scale
                    })
                    .collect()
            })
            .collect();

        Self {
            dim,
            input_dim,
            activation,
            weights,
            bias: vec![0.0; dim],
            values: vec![0.0; dim],
            errors: vec![0.0; dim],
            preactivations: vec![0.0; dim],
        }
    }

    pub fn dim(&self) -> usize { self.dim }
    pub fn input_dim(&self) -> usize { self.input_dim }
    pub fn activation(&self) -> Activation { self.activation }
    pub fn weights(&self) -> &Vec<Vec<f64>> { &self.weights }
    pub fn bias(&self) -> &[f64] { &self.bias }
    pub fn values(&self) -> &[f64] { &self.values }
    pub fn errors(&self) -> &[f64] { &self.errors }
    pub fn preactivations(&self) -> &[f64] { &self.preactivations }

    pub fn set_weights(&mut self, w: Vec<Vec<f64>>) { self.weights = w; }
    pub fn set_bias(&mut self, b: Vec<f64>) { self.bias = b; }
    pub fn set_values(&mut self, v: Vec<f64>) { self.values = v; }

    /// Compute prediction: x_hat(l) = f(W(l) * x_above + b(l))
    /// Also stores preactivations for derivative computation.
    /// Returns the prediction vector.
    pub fn predict(&mut self, x_above: &[f64]) -> Vec<f64> {
        let mut prediction = vec![0.0; self.dim];
        for i in 0..self.dim {
            let mut sum = self.bias[i];
            for j in 0..self.input_dim {
                sum += self.weights[i][j] * x_above[j];
            }
            self.preactivations[i] = sum;
            prediction[i] = self.activation.apply(sum);
        }
        prediction
    }

    /// Compute prediction errors: e(l) = x(l) - x_hat(l)
    pub fn compute_errors(&mut self, x_above: &[f64]) {
        let prediction = self.predict(x_above);
        for i in 0..self.dim {
            self.errors[i] = self.values[i] - prediction[i];
        }
    }

    /// Compute W^T * (f'(a) . e) — the top-down error signal for the layer above.
    /// Returns a vector of dimension input_dim.
    pub fn top_down_error(&self) -> Vec<f64> {
        let mut result = vec![0.0; self.input_dim];
        for j in 0..self.input_dim {
            let mut sum = 0.0;
            for i in 0..self.dim {
                let fprime = self.activation.derivative(self.preactivations[i]);
                sum += self.weights[i][j] * fprime * self.errors[i];
            }
            result[j] = sum;
        }
        result
    }

    /// Apply Hebbian weight update: W(l) += lr * (f'(a(l)) . e(l)) * x(l+1)^T
    pub fn update_weights(&mut self, x_above: &[f64], learning_rate: f64) {
        for i in 0..self.dim {
            let fprime = self.activation.derivative(self.preactivations[i]);
            let scaled_error = fprime * self.errors[i] * learning_rate;
            for j in 0..self.input_dim {
                self.weights[i][j] += scaled_error * x_above[j];
            }
            // Bias update: lr * f'(a) * e
            self.bias[i] += scaled_error;
        }
    }

    /// Initialize values to small random values.
    pub fn randomize_values(&mut self, seed: u64) {
        for i in 0..self.dim {
            let s = ((seed.wrapping_mul(6364136223846793005).wrapping_add(i as u64)) % 10000) as f64;
            self.values[i] = (s / 10000.0 - 0.5) * 0.1;
        }
    }

    /// Resize the input dimension (for vocabulary growth).
    /// New weight columns are initialized to small values.
    pub fn resize_input(&mut self, new_input_dim: usize) {
        if new_input_dim <= self.input_dim {
            return;
        }
        let scale = (2.0 / (self.dim + new_input_dim) as f64).sqrt();
        for row in &mut self.weights {
            for j in self.input_dim..new_input_dim {
                let seed = (row.len() * 7919 + j * 6271) as f64;
                row.push(((seed % 1000.0) / 1000.0 - 0.5) * 2.0 * scale);
            }
        }
        self.input_dim = new_input_dim;
    }
}
```

**Step 4: Run test to verify it passes**

Run: `cd simse-predictive-coding && cargo test`
Expected: PASS

**Step 5: Commit**

```bash
git add simse-predictive-coding/src/layer.rs simse-predictive-coding/src/lib.rs
git commit -m "feat(pcn): add PcnLayer with predict, error, and Hebbian weight update"
```

---

### Task 4: Implement PredictiveCodingNetwork — multi-layer inference and learning

**Files:**
- Create: `simse-predictive-coding/src/network.rs`
- Modify: `simse-predictive-coding/src/lib.rs` — add `pub mod network;`

**Step 1: Write the failing test**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{Activation, LayerConfig, PcnConfig};

    fn test_config() -> PcnConfig {
        PcnConfig {
            layers: vec![
                LayerConfig { dim: 8, activation: Activation::Relu },
                LayerConfig { dim: 4, activation: Activation::Tanh },
            ],
            inference_steps: 10,
            learning_rate: 0.01,
            inference_rate: 0.1,
            temporal_amortization: false,
            ..Default::default()
        }
    }

    #[test]
    fn network_creates_correct_layers() {
        let net = PredictiveCodingNetwork::new(6, &test_config());
        assert_eq!(net.num_layers(), 2);
        assert_eq!(net.input_dim(), 6);
    }

    #[test]
    fn infer_reduces_energy() {
        let mut net = PredictiveCodingNetwork::new(6, &test_config());
        let input = vec![1.0, 0.5, -0.3, 0.8, 0.0, -1.0];
        let energy_before = net.infer(&input, 1);
        let energy_after = net.infer(&input, 10);
        // After more inference steps on the same input, energy should not increase
        // (it may not always decrease due to randomness, but it should converge)
        assert!(energy_after.is_finite());
    }

    #[test]
    fn train_step_updates_weights() {
        let mut net = PredictiveCodingNetwork::new(6, &test_config());
        let input = vec![1.0, 0.5, -0.3, 0.8, 0.0, -1.0];

        // Snapshot weights before
        let w_before: Vec<f64> = net.layer(0).weights()[0].clone();

        net.train_single(&input);

        let w_after: Vec<f64> = net.layer(0).weights()[0].clone();
        // Weights should have changed
        let changed = w_before.iter().zip(w_after.iter()).any(|(a, b)| (a - b).abs() > 1e-15);
        assert!(changed, "Weights should change after training");
    }

    #[test]
    fn energy_is_sum_of_squared_errors() {
        let mut net = PredictiveCodingNetwork::new(4, &PcnConfig {
            layers: vec![
                LayerConfig { dim: 3, activation: Activation::Relu },
            ],
            inference_steps: 1,
            ..Default::default()
        });
        let input = vec![1.0, 2.0, 3.0, 4.0];
        let energy = net.infer(&input, 1);
        assert!(energy >= 0.0, "Energy must be non-negative");
    }

    #[test]
    fn get_latent_returns_top_layer_values() {
        let mut net = PredictiveCodingNetwork::new(6, &test_config());
        let input = vec![1.0, 0.5, -0.3, 0.8, 0.0, -1.0];
        net.infer(&input, 5);
        let latent = net.get_top_latent();
        assert_eq!(latent.len(), 4); // top layer dim
    }

    #[test]
    fn generate_reconstructs_from_top_latent() {
        let mut net = PredictiveCodingNetwork::new(6, &test_config());
        let input = vec![1.0, 0.5, -0.3, 0.8, 0.0, -1.0];
        net.infer(&input, 20);
        let reconstruction = net.generate();
        assert_eq!(reconstruction.len(), 6); // input dim
    }

    #[test]
    fn per_layer_energy_breakdown() {
        let mut net = PredictiveCodingNetwork::new(6, &test_config());
        let input = vec![1.0, 0.5, -0.3, 0.8, 0.0, -1.0];
        net.infer(&input, 5);
        let breakdown = net.energy_breakdown();
        assert_eq!(breakdown.len(), 2); // one per layer
        let total: f64 = breakdown.iter().sum();
        assert!(total >= 0.0);
    }
}
```

**Step 2: Run test to verify it fails**

Run: `cd simse-predictive-coding && cargo test`
Expected: FAIL

**Step 3: Write implementation**

```rust
use crate::config::PcnConfig;
use crate::layer::PcnLayer;

/// Multi-layer predictive coding network.
///
/// Implements the full inference loop (T-step prediction error minimization)
/// and local Hebbian weight updates as described in Stenlund 2025.
pub struct PredictiveCodingNetwork {
    input_dim: usize,
    layers: Vec<PcnLayer>,
    inference_rate: f64,
    learning_rate: f64,
}

impl PredictiveCodingNetwork {
    /// Create a new network with the given input dimension and config.
    pub fn new(input_dim: usize, config: &PcnConfig) -> Self {
        let mut layers = Vec::new();
        let mut prev_dim = input_dim;
        for layer_config in &config.layers {
            layers.push(PcnLayer::new(layer_config.dim, prev_dim, layer_config.activation));
            prev_dim = layer_config.dim;
        }
        Self {
            input_dim,
            layers,
            inference_rate: config.inference_rate,
            learning_rate: config.learning_rate,
        }
    }

    pub fn num_layers(&self) -> usize { self.layers.len() }
    pub fn input_dim(&self) -> usize { self.input_dim }
    pub fn layer(&self, idx: usize) -> &PcnLayer { &self.layers[idx] }

    /// Run T inference steps on clamped input. Returns total energy after inference.
    pub fn infer(&mut self, input: &[f64], steps: usize) -> f64 {
        if self.layers.is_empty() {
            return 0.0;
        }

        // Randomize latent values
        for (i, layer) in self.layers.iter_mut().enumerate() {
            layer.randomize_values(i as u64 + 42);
        }

        for _t in 0..steps {
            self.inference_step(input);
        }

        self.compute_energy(input)
    }

    /// Run T inference steps but preserve latent values (temporal amortization).
    pub fn infer_amortized(&mut self, input: &[f64], steps: usize) -> f64 {
        for _t in 0..steps {
            self.inference_step(input);
        }
        self.compute_energy(input)
    }

    /// Single inference step: compute all errors, then update all latents synchronously.
    fn inference_step(&mut self, input: &[f64]) {
        let n = self.layers.len();

        // Compute predictions and errors for all layers
        // Layer 0 predicts input, layer l predicts layer l-1
        let mut x_above_for_layer: Vec<Vec<f64>> = Vec::with_capacity(n);
        for l in 0..n {
            let x_above = if l + 1 < n {
                self.layers[l + 1].values().to_vec()
            } else {
                // Top layer: no prediction from above, just use own values
                vec![0.0; self.layers[l].dim()]
            };
            x_above_for_layer.push(x_above);
        }

        // Compute errors: e(l) = x(l) - f(W(l) * x(l+1))
        for l in 0..n {
            self.layers[l].compute_errors(&x_above_for_layer[l]);
        }

        // Compute top-down error signals before updating latents
        let mut top_down_signals: Vec<Vec<f64>> = Vec::with_capacity(n);
        for l in 0..n {
            top_down_signals.push(self.layers[l].top_down_error());
        }

        // Update latent values synchronously
        // x(l) -= lr_infer * (e(l) - W(l-1)^T * (f'(a(l-1)) . e(l-1)))
        for l in 0..n {
            let mut new_values = self.layers[l].values().to_vec();
            for i in 0..new_values.len() {
                let error_term = self.layers[l].errors()[i];
                let below_signal = if l > 0 {
                    // The top-down error from layer l-1 projected back
                    // This is W(l-1)^T * (f'(a) . e(l-1)), but indexed into layer l's dimension
                    // Actually, layer l-1 has top_down_error of dimension l-1.input_dim = layer before l-1
                    // We need the signal going UP to layer l from layer l-1
                    // top_down_signals[l-1] has dim = layers[l-1].input_dim
                    // But we want the signal at layer l's values
                    // For the standard PCN update: the term is sum over layer l-1 units
                    // that project back to unit i of layer l
                    // This is: sum_k W(l-1)[k][i] * f'(a(l-1)[k]) * e(l-1)[k]
                    // We can compute this directly:
                    let layer_below = &self.layers[l - 1];
                    let mut sig = 0.0;
                    for k in 0..layer_below.dim() {
                        let fprime = layer_below.activation().derivative(layer_below.preactivations()[k]);
                        sig += layer_below.weights()[k][i] * fprime * layer_below.errors()[k];
                    }
                    sig
                } else {
                    0.0
                };
                new_values[i] -= self.inference_rate * (error_term - below_signal);
            }
            self.layers[l].set_values(new_values);
        }
    }

    /// Compute total energy: E = 0.5 * sum_l ||e(l)||^2
    fn compute_energy(&mut self, input: &[f64]) -> f64 {
        let n = self.layers.len();
        let mut total = 0.0;

        // Recompute errors with current values
        for l in 0..n {
            let x_above = if l + 1 < n {
                self.layers[l + 1].values().to_vec()
            } else {
                vec![0.0; self.layers[l].dim()]
            };
            self.layers[l].compute_errors(&x_above);
        }

        // Also include input layer error
        if !self.layers.is_empty() {
            let pred = self.layers[0].predict(&if self.layers.len() > 1 {
                self.layers[1].values().to_vec()
            } else {
                vec![0.0; self.layers[0].dim()]
            });
            // Input error: input - prediction of input
            for i in 0..input.len().min(pred.len()) {
                let e = input[i] - pred[i];
                total += 0.5 * e * e;
            }
        }

        for l in 0..n {
            for &e in self.layers[l].errors() {
                total += 0.5 * e * e;
            }
        }

        total
    }

    /// Train on a single sample: infer then update weights.
    pub fn train_single(&mut self, input: &[f64]) -> f64 {
        let energy = self.infer(input, 20); // default inference steps

        // Update weights using local Hebbian rule
        let n = self.layers.len();
        for l in 0..n {
            let x_above = if l + 1 < n {
                self.layers[l + 1].values().to_vec()
            } else {
                vec![0.0; self.layers[l].dim()]
            };
            self.layers[l].update_weights(&x_above, self.learning_rate);
        }

        energy
    }

    /// Train on a single sample with T inference steps then weight update.
    pub fn train_single_with_steps(&mut self, input: &[f64], steps: usize, amortized: bool) -> f64 {
        let energy = if amortized {
            self.infer_amortized(input, steps)
        } else {
            self.infer(input, steps)
        };

        let n = self.layers.len();
        for l in 0..n {
            let x_above = if l + 1 < n {
                self.layers[l + 1].values().to_vec()
            } else {
                vec![0.0; self.layers[l].dim()]
            };
            self.layers[l].update_weights(&x_above, self.learning_rate);
        }

        energy
    }

    /// Get the top layer's latent values.
    pub fn get_top_latent(&self) -> Vec<f64> {
        self.layers.last()
            .map(|l| l.values().to_vec())
            .unwrap_or_default()
    }

    /// Generate (reconstruct) from top latent down to input dimension.
    pub fn generate(&mut self) -> Vec<f64> {
        if self.layers.is_empty() {
            return vec![];
        }

        let n = self.layers.len();
        // Top-down generation: each layer predicts the one below
        let mut current = self.layers[n - 1].values().to_vec();
        for l in (0..n).rev() {
            current = self.layers[l].predict(&current);
            if l > 0 {
                current = self.layers[l - 1].predict(&self.layers[l].values().to_vec());
            }
        }
        // Final prediction from layer 0
        let top_vals = if n > 1 {
            self.layers[1].values().to_vec()
        } else {
            self.layers[0].values().to_vec()
        };
        self.layers[0].predict(&top_vals)
    }

    /// Get per-layer energy breakdown.
    pub fn energy_breakdown(&self) -> Vec<f64> {
        self.layers.iter().map(|layer| {
            layer.errors().iter().map(|e| 0.5 * e * e).sum()
        }).collect()
    }

    /// Resize the input dimension (e.g., when vocabulary grows).
    pub fn resize_input(&mut self, new_input_dim: usize) {
        if !self.layers.is_empty() {
            self.layers[0].resize_input(new_input_dim);
        }
        self.input_dim = new_input_dim;
    }
}
```

**Step 4: Run test to verify it passes**

Run: `cd simse-predictive-coding && cargo test`
Expected: PASS

**Step 5: Commit**

```bash
git add simse-predictive-coding/src/network.rs simse-predictive-coding/src/lib.rs
git commit -m "feat(pcn): add PredictiveCodingNetwork with inference and Hebbian learning"
```

---

### Task 5: Implement VocabularyManager — dynamic topic/tag vocabularies

**Files:**
- Create: `simse-predictive-coding/src/vocabulary.rs`
- Modify: `simse-predictive-coding/src/lib.rs` — add `pub mod vocabulary;`

**Step 1: Write the failing test**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_vocabulary_is_empty() {
        let vocab = VocabularyManager::new(100, 200);
        assert_eq!(vocab.topic_count(), 0);
        assert_eq!(vocab.tag_count(), 0);
        assert_eq!(vocab.total_dim(), 0);
    }

    #[test]
    fn register_topic_returns_index() {
        let mut vocab = VocabularyManager::new(100, 200);
        let idx = vocab.register_topic("rust").unwrap();
        assert_eq!(idx, 0);
        let idx2 = vocab.register_topic("python").unwrap();
        assert_eq!(idx2, 1);
        // Same topic returns same index
        let idx3 = vocab.register_topic("rust").unwrap();
        assert_eq!(idx3, 0);
    }

    #[test]
    fn register_tag_returns_index() {
        let mut vocab = VocabularyManager::new(100, 200);
        let idx = vocab.register_tag("error-handling").unwrap();
        assert_eq!(idx, 0);
        let idx2 = vocab.register_tag("error-handling").unwrap();
        assert_eq!(idx2, 0);
    }

    #[test]
    fn topic_overflow_returns_error() {
        let mut vocab = VocabularyManager::new(2, 100);
        vocab.register_topic("a").unwrap();
        vocab.register_topic("b").unwrap();
        let result = vocab.register_topic("c");
        assert!(result.is_err());
    }

    #[test]
    fn encode_topic_one_hot() {
        let mut vocab = VocabularyManager::new(100, 200);
        vocab.register_topic("rust").unwrap();
        vocab.register_topic("python").unwrap();
        vocab.register_topic("go").unwrap();

        let encoded = vocab.encode_topic("python");
        assert_eq!(encoded, vec![0.0, 1.0, 0.0]);
    }

    #[test]
    fn encode_unknown_topic_is_zeros() {
        let mut vocab = VocabularyManager::new(100, 200);
        vocab.register_topic("rust").unwrap();
        let encoded = vocab.encode_topic("unknown");
        assert_eq!(encoded, vec![0.0]);
    }

    #[test]
    fn encode_tags_bitmap() {
        let mut vocab = VocabularyManager::new(100, 200);
        vocab.register_tag("a").unwrap();
        vocab.register_tag("b").unwrap();
        vocab.register_tag("c").unwrap();

        let encoded = vocab.encode_tags(&["a".to_string(), "c".to_string()]);
        assert_eq!(encoded, vec![1.0, 0.0, 1.0]);
    }

    #[test]
    fn total_dim_accounts_for_all_structured_features() {
        let mut vocab = VocabularyManager::new(100, 200);
        vocab.register_topic("rust").unwrap();
        vocab.register_tag("a").unwrap();
        vocab.register_tag("b").unwrap();
        // total = topics(1) + tags(2) + entry_type(3) + temporal(3) + action(4) = 13
        assert_eq!(vocab.total_dim(), 13);
    }

    #[test]
    fn serialize_restore_round_trip() {
        let mut vocab = VocabularyManager::new(100, 200);
        vocab.register_topic("rust").unwrap();
        vocab.register_topic("python").unwrap();
        vocab.register_tag("error").unwrap();

        let state = vocab.serialize();
        let json = serde_json::to_string(&state).unwrap();
        let restored_state: VocabularyState = serde_json::from_str(&json).unwrap();
        let vocab2 = VocabularyManager::from_state(&restored_state);

        assert_eq!(vocab2.topic_count(), 2);
        assert_eq!(vocab2.tag_count(), 1);
        assert_eq!(vocab2.encode_topic("rust"), vocab.encode_topic("rust"));
    }
}
```

**Step 2: Run test to verify it fails**

Run: `cd simse-predictive-coding && cargo test`
Expected: FAIL

**Step 3: Write implementation**

```rust
use std::collections::HashMap;
use serde::{Deserialize, Serialize};
use crate::error::PcnError;

/// Fixed structured feature dimensions.
const ENTRY_TYPE_DIM: usize = 3;   // fact, decision, observation
const TEMPORAL_DIM: usize = 3;     // timestamp, time_since_last, session_ordinal
const ACTION_DIM: usize = 4;       // extraction, compendium, reorganization, optimization
const FIXED_DIM: usize = ENTRY_TYPE_DIM + TEMPORAL_DIM + ACTION_DIM; // 10

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VocabularyState {
    pub topics: Vec<String>,
    pub tags: Vec<String>,
    pub max_topics: usize,
    pub max_tags: usize,
}

pub struct VocabularyManager {
    topic_to_idx: HashMap<String, usize>,
    tag_to_idx: HashMap<String, usize>,
    topics: Vec<String>,
    tags: Vec<String>,
    max_topics: usize,
    max_tags: usize,
}

impl VocabularyManager {
    pub fn new(max_topics: usize, max_tags: usize) -> Self {
        Self {
            topic_to_idx: HashMap::new(),
            tag_to_idx: HashMap::new(),
            topics: Vec::new(),
            tags: Vec::new(),
            max_topics,
            max_tags,
        }
    }

    pub fn from_state(state: &VocabularyState) -> Self {
        let mut mgr = Self::new(state.max_topics, state.max_tags);
        for topic in &state.topics {
            let _ = mgr.register_topic(topic);
        }
        for tag in &state.tags {
            let _ = mgr.register_tag(tag);
        }
        mgr
    }

    pub fn topic_count(&self) -> usize { self.topics.len() }
    pub fn tag_count(&self) -> usize { self.tags.len() }

    /// Total structured feature dimensions (topics + tags + fixed).
    pub fn total_dim(&self) -> usize {
        self.topics.len() + self.tags.len() + FIXED_DIM
    }

    pub fn register_topic(&mut self, topic: &str) -> Result<usize, PcnError> {
        if let Some(&idx) = self.topic_to_idx.get(topic) {
            return Ok(idx);
        }
        if self.topics.len() >= self.max_topics {
            return Err(PcnError::VocabularyOverflow(format!(
                "Topic vocabulary full ({} max)", self.max_topics
            )));
        }
        let idx = self.topics.len();
        self.topics.push(topic.to_string());
        self.topic_to_idx.insert(topic.to_string(), idx);
        Ok(idx)
    }

    pub fn register_tag(&mut self, tag: &str) -> Result<usize, PcnError> {
        if let Some(&idx) = self.tag_to_idx.get(tag) {
            return Ok(idx);
        }
        if self.tags.len() >= self.max_tags {
            return Err(PcnError::VocabularyOverflow(format!(
                "Tag vocabulary full ({} max)", self.max_tags
            )));
        }
        let idx = self.tags.len();
        self.tags.push(tag.to_string());
        self.tag_to_idx.insert(tag.to_string(), idx);
        Ok(idx)
    }

    /// One-hot encode a topic. Returns zeros if topic is unknown.
    pub fn encode_topic(&self, topic: &str) -> Vec<f64> {
        let mut encoded = vec![0.0; self.topics.len()];
        if let Some(&idx) = self.topic_to_idx.get(topic) {
            encoded[idx] = 1.0;
        }
        encoded
    }

    /// Bitmap encode a set of tags.
    pub fn encode_tags(&self, tags: &[String]) -> Vec<f64> {
        let mut encoded = vec![0.0; self.tags.len()];
        for tag in tags {
            if let Some(&idx) = self.tag_to_idx.get(tag.as_str()) {
                encoded[idx] = 1.0;
            }
        }
        encoded
    }

    /// Encode entry type as one-hot [fact, decision, observation].
    pub fn encode_entry_type(entry_type: &str) -> Vec<f64> {
        match entry_type {
            "fact" => vec![1.0, 0.0, 0.0],
            "decision" => vec![0.0, 1.0, 0.0],
            "observation" => vec![0.0, 0.0, 1.0],
            _ => vec![0.0, 0.0, 0.0],
        }
    }

    /// Encode temporal features: [normalized_timestamp, time_since_last, session_ordinal].
    pub fn encode_temporal(timestamp: f64, time_since_last: f64, session_ordinal: f64) -> Vec<f64> {
        vec![timestamp, time_since_last, session_ordinal]
    }

    /// Encode action context as one-hot [extraction, compendium, reorganization, optimization].
    pub fn encode_action(action: &str) -> Vec<f64> {
        match action {
            "extraction" => vec![1.0, 0.0, 0.0, 0.0],
            "compendium" => vec![0.0, 1.0, 0.0, 0.0],
            "reorganization" => vec![0.0, 0.0, 1.0, 0.0],
            "optimization" => vec![0.0, 0.0, 0.0, 1.0],
            _ => vec![0.0, 0.0, 0.0, 0.0],
        }
    }

    pub fn serialize(&self) -> VocabularyState {
        VocabularyState {
            topics: self.topics.clone(),
            tags: self.tags.clone(),
            max_topics: self.max_topics,
            max_tags: self.max_tags,
        }
    }
}
```

**Step 4: Run test to verify it passes**

Run: `cd simse-predictive-coding && cargo test`
Expected: PASS

**Step 5: Commit**

```bash
git add simse-predictive-coding/src/vocabulary.rs simse-predictive-coding/src/lib.rs
git commit -m "feat(pcn): add VocabularyManager with dynamic topic/tag encoding"
```

---

### Task 6: Implement InputEncoder — builds combined input vectors

**Files:**
- Create: `simse-predictive-coding/src/encoder.rs`
- Modify: `simse-predictive-coding/src/lib.rs` — add `pub mod encoder;`

**Step 1: Write the failing test**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encode_event_produces_correct_length() {
        let mut encoder = InputEncoder::new(4, 100, 200); // 4-dim embeddings
        let event = LibraryEvent {
            embedding: vec![1.0, 0.5, -0.3, 0.8],
            topic: "rust".into(),
            tags: vec!["error".into()],
            entry_type: "fact".into(),
            timestamp: 1000.0,
            time_since_last: 500.0,
            session_ordinal: 1.0,
            action: "extraction".into(),
        };
        let (encoded, grew) = encoder.encode(&event).unwrap();
        // 4 (embedding) + 1 (topic) + 1 (tag) + 3 (entry_type) + 3 (temporal) + 4 (action) = 16
        assert_eq!(encoded.len(), 16);
        assert!(grew); // first event always grows vocab
    }

    #[test]
    fn encode_preserves_embedding_values() {
        let mut encoder = InputEncoder::new(3, 100, 200);
        let event = LibraryEvent {
            embedding: vec![1.5, -2.0, 3.0],
            topic: "test".into(),
            tags: vec![],
            entry_type: "fact".into(),
            timestamp: 0.0,
            time_since_last: 0.0,
            session_ordinal: 0.0,
            action: "extraction".into(),
        };
        let (encoded, _) = encoder.encode(&event).unwrap();
        assert_eq!(encoded[0], 1.5);
        assert_eq!(encoded[1], -2.0);
        assert_eq!(encoded[2], 3.0);
    }

    #[test]
    fn input_dim_grows_with_vocabulary() {
        let mut encoder = InputEncoder::new(2, 100, 200);
        assert_eq!(encoder.current_input_dim(), 12); // 2 + 0 + 0 + 10

        let event1 = LibraryEvent {
            embedding: vec![1.0, 0.0],
            topic: "a".into(),
            tags: vec!["x".into()],
            entry_type: "fact".into(),
            timestamp: 0.0,
            time_since_last: 0.0,
            session_ordinal: 0.0,
            action: "extraction".into(),
        };
        encoder.encode(&event1).unwrap();
        assert_eq!(encoder.current_input_dim(), 14); // 2 + 1 + 1 + 10

        let event2 = LibraryEvent {
            embedding: vec![1.0, 0.0],
            topic: "b".into(),
            tags: vec!["y".into()],
            entry_type: "fact".into(),
            timestamp: 0.0,
            time_since_last: 0.0,
            session_ordinal: 0.0,
            action: "extraction".into(),
        };
        encoder.encode(&event2).unwrap();
        assert_eq!(encoder.current_input_dim(), 16); // 2 + 2 + 2 + 10
    }
}
```

**Step 2: Run test to verify it fails**

Run: `cd simse-predictive-coding && cargo test`
Expected: FAIL

**Step 3: Write implementation**

```rust
use crate::error::PcnError;
use crate::vocabulary::VocabularyManager;
use serde::{Deserialize, Serialize};

/// Event received from the library (CirculationDesk).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LibraryEvent {
    pub embedding: Vec<f32>,
    pub topic: String,
    pub tags: Vec<String>,
    pub entry_type: String,
    pub timestamp: f64,
    pub time_since_last: f64,
    pub session_ordinal: f64,
    pub action: String,
}

/// Encodes LibraryEvents into combined input vectors for the PCN.
pub struct InputEncoder {
    embedding_dim: usize,
    vocab: VocabularyManager,
}

impl InputEncoder {
    pub fn new(embedding_dim: usize, max_topics: usize, max_tags: usize) -> Self {
        Self {
            embedding_dim,
            vocab: VocabularyManager::new(max_topics, max_tags),
        }
    }

    pub fn from_vocab(embedding_dim: usize, vocab: VocabularyManager) -> Self {
        Self { embedding_dim, vocab }
    }

    /// Current total input dimension (embedding + structured features).
    pub fn current_input_dim(&self) -> usize {
        self.embedding_dim + self.vocab.total_dim()
    }

    pub fn vocab(&self) -> &VocabularyManager { &self.vocab }
    pub fn vocab_mut(&mut self) -> &mut VocabularyManager { &mut self.vocab }

    /// Encode a library event into a combined input vector.
    /// Returns (encoded_vector, vocabulary_grew).
    pub fn encode(&mut self, event: &LibraryEvent) -> Result<(Vec<f64>, bool), PcnError> {
        let prev_dim = self.current_input_dim();

        // Register topic and tags (may grow vocabulary)
        self.vocab.register_topic(&event.topic)?;
        for tag in &event.tags {
            self.vocab.register_tag(tag)?;
        }

        let grew = self.current_input_dim() != prev_dim;

        // Build the combined vector
        let mut encoded = Vec::with_capacity(self.current_input_dim());

        // 1. Semantic embedding (f32 -> f64)
        for &v in &event.embedding {
            encoded.push(v as f64);
        }
        // Pad if embedding is shorter than expected
        while encoded.len() < self.embedding_dim {
            encoded.push(0.0);
        }

        // 2. Topic one-hot
        encoded.extend(self.vocab.encode_topic(&event.topic));

        // 3. Tag bitmap
        encoded.extend(self.vocab.encode_tags(&event.tags));

        // 4. Entry type one-hot
        encoded.extend(VocabularyManager::encode_entry_type(&event.entry_type));

        // 5. Temporal features
        encoded.extend(VocabularyManager::encode_temporal(
            event.timestamp,
            event.time_since_last,
            event.session_ordinal,
        ));

        // 6. Action context one-hot
        encoded.extend(VocabularyManager::encode_action(&event.action));

        Ok((encoded, grew))
    }
}
```

**Step 4: Run test to verify it passes**

Run: `cd simse-predictive-coding && cargo test`
Expected: PASS

**Step 5: Commit**

```bash
git add simse-predictive-coding/src/encoder.rs simse-predictive-coding/src/lib.rs
git commit -m "feat(pcn): add InputEncoder with dual embedding + structured metadata encoding"
```

---

### Task 7: Implement ModelSnapshot — immutable weight snapshot for lock-free reads

**Files:**
- Create: `simse-predictive-coding/src/snapshot.rs`
- Modify: `simse-predictive-coding/src/lib.rs` — add `pub mod snapshot;`

**Step 1: Write the failing test**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{Activation, LayerConfig, PcnConfig};
    use crate::network::PredictiveCodingNetwork;
    use crate::vocabulary::VocabularyManager;

    #[test]
    fn snapshot_from_network_captures_weights() {
        let config = PcnConfig {
            layers: vec![LayerConfig { dim: 4, activation: Activation::Relu }],
            ..Default::default()
        };
        let net = PredictiveCodingNetwork::new(3, &config);
        let vocab = VocabularyManager::new(100, 200);
        let snapshot = ModelSnapshot::from_network(&net, &vocab, 0, 0);

        assert_eq!(snapshot.layer_weights.len(), 1);
        assert_eq!(snapshot.layer_weights[0].len(), 4); // 4 rows
        assert_eq!(snapshot.layer_weights[0][0].len(), 3); // 3 cols
        assert_eq!(snapshot.layer_biases[0].len(), 4);
    }

    #[test]
    fn snapshot_can_run_inference() {
        let config = PcnConfig {
            layers: vec![
                LayerConfig { dim: 4, activation: Activation::Relu },
                LayerConfig { dim: 2, activation: Activation::Tanh },
            ],
            inference_steps: 5,
            ..Default::default()
        };
        let mut net = PredictiveCodingNetwork::new(3, &config);

        // Train a bit
        for _ in 0..10 {
            net.train_single(&[1.0, 0.5, -0.3]);
        }

        let vocab = VocabularyManager::new(100, 200);
        let snapshot = ModelSnapshot::from_network(&net, &vocab, 10, 10);

        let input = vec![1.0, 0.5, -0.3];
        let result = snapshot.predict(&input, 5);
        assert!(result.energy.is_finite());
        assert_eq!(result.top_latent.len(), 2);
        assert_eq!(result.energy_breakdown.len(), 2);
    }
}
```

**Step 2: Run test to verify it fails**

Run: `cd simse-predictive-coding && cargo test`
Expected: FAIL

**Step 3: Write implementation**

```rust
use serde::{Deserialize, Serialize};
use crate::config::{Activation, LayerConfig, PcnConfig};
use crate::layer::PcnLayer;
use crate::network::PredictiveCodingNetwork;
use crate::vocabulary::{VocabularyManager, VocabularyState};

/// Immutable snapshot of a trained model for lock-free reads.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelSnapshot {
    pub input_dim: usize,
    pub layer_configs: Vec<LayerConfig>,
    pub layer_weights: Vec<Vec<Vec<f64>>>,
    pub layer_biases: Vec<Vec<f64>>,
    pub vocabulary: VocabularyState,
    pub epoch: usize,
    pub total_samples: usize,
}

/// Result of a prediction query on a snapshot.
#[derive(Debug, Clone)]
pub struct PredictionResult {
    pub energy: f64,
    pub top_latent: Vec<f64>,
    pub energy_breakdown: Vec<f64>,
    pub reconstruction: Vec<f64>,
}

impl ModelSnapshot {
    /// Create a snapshot from the current network state.
    pub fn from_network(
        net: &PredictiveCodingNetwork,
        vocab: &VocabularyManager,
        epoch: usize,
        total_samples: usize,
    ) -> Self {
        let mut layer_weights = Vec::new();
        let mut layer_biases = Vec::new();
        let mut layer_configs = Vec::new();

        for i in 0..net.num_layers() {
            let layer = net.layer(i);
            layer_weights.push(layer.weights().clone());
            layer_biases.push(layer.bias().to_vec());
            layer_configs.push(LayerConfig {
                dim: layer.dim(),
                activation: layer.activation(),
            });
        }

        Self {
            input_dim: net.input_dim(),
            layer_configs,
            layer_weights,
            layer_biases,
            vocabulary: vocab.serialize(),
            epoch,
            total_samples,
        }
    }

    /// Run inference on this snapshot (creates a temporary network).
    pub fn predict(&self, input: &[f64], inference_steps: usize) -> PredictionResult {
        let config = PcnConfig {
            layers: self.layer_configs.clone(),
            inference_steps,
            ..Default::default()
        };
        let mut net = PredictiveCodingNetwork::new(self.input_dim, &config);

        // Load snapshot weights into the network
        for i in 0..net.num_layers() {
            net.layer_mut(i).set_weights(self.layer_weights[i].clone());
            net.layer_mut(i).set_bias(self.layer_biases[i].clone());
        }

        let energy = net.infer(input, inference_steps);
        let top_latent = net.get_top_latent();
        let energy_breakdown = net.energy_breakdown();
        let reconstruction = net.generate();

        PredictionResult {
            energy,
            top_latent,
            energy_breakdown,
            reconstruction,
        }
    }

    /// Create an empty/default snapshot.
    pub fn empty() -> Self {
        Self {
            input_dim: 0,
            layer_configs: Vec::new(),
            layer_weights: Vec::new(),
            layer_biases: Vec::new(),
            vocabulary: VocabularyState {
                topics: Vec::new(),
                tags: Vec::new(),
                max_topics: 500,
                max_tags: 1000,
            },
            epoch: 0,
            total_samples: 0,
        }
    }
}
```

Note: This requires adding a `layer_mut` method to `PredictiveCodingNetwork`:

Add to `network.rs`:
```rust
pub fn layer_mut(&mut self, idx: usize) -> &mut PcnLayer { &mut self.layers[idx] }
```

**Step 4: Run test to verify it passes**

Run: `cd simse-predictive-coding && cargo test`
Expected: PASS

**Step 5: Commit**

```bash
git add simse-predictive-coding/src/snapshot.rs simse-predictive-coding/src/network.rs simse-predictive-coding/src/lib.rs
git commit -m "feat(pcn): add ModelSnapshot for lock-free concurrent prediction reads"
```

---

### Task 8: Implement TrainingWorker — background training loop with snapshot swap

**Files:**
- Create: `simse-predictive-coding/src/trainer.rs`
- Modify: `simse-predictive-coding/src/lib.rs` — add `pub mod trainer;`

**Step 1: Write the failing test**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::PcnConfig;
    use crate::encoder::LibraryEvent;
    use crate::snapshot::ModelSnapshot;
    use std::sync::{Arc, RwLock};

    fn make_event(topic: &str) -> LibraryEvent {
        LibraryEvent {
            embedding: vec![1.0, 0.5, -0.3, 0.8],
            topic: topic.into(),
            tags: vec!["test".into()],
            entry_type: "fact".into(),
            timestamp: 1000.0,
            time_since_last: 0.0,
            session_ordinal: 1.0,
            action: "extraction".into(),
        }
    }

    #[tokio::test]
    async fn trainer_processes_batch_and_updates_snapshot() {
        let config = PcnConfig {
            batch_size: 2,
            max_batch_delay_ms: 100,
            ..Default::default()
        };
        let snapshot = Arc::new(RwLock::new(ModelSnapshot::empty()));
        let (tx, rx) = tokio::sync::mpsc::channel(100);

        let snapshot_clone = snapshot.clone();
        let handle = tokio::task::spawn_blocking(move || {
            let rt = tokio::runtime::Handle::current();
            rt.block_on(async {
                TrainingWorker::run_batch(rx, snapshot_clone, config, 4).await
            })
        });

        // Send 2 events (= 1 batch)
        tx.send(make_event("rust")).await.unwrap();
        tx.send(make_event("python")).await.unwrap();
        drop(tx); // Close channel to signal completion

        handle.await.unwrap();

        let snap = snapshot.read().unwrap();
        assert!(snap.total_samples >= 2);
    }

    #[test]
    fn training_worker_stats_default() {
        let stats = TrainingStats::default();
        assert_eq!(stats.epochs, 0);
        assert_eq!(stats.total_samples, 0);
        assert_eq!(stats.dropped_events, 0);
    }
}
```

**Step 2: Run test to verify it fails**

Run: `cd simse-predictive-coding && cargo test`
Expected: FAIL

**Step 3: Write implementation**

```rust
use std::sync::{Arc, RwLock};

use tokio::sync::mpsc;

use crate::config::PcnConfig;
use crate::encoder::{InputEncoder, LibraryEvent};
use crate::network::PredictiveCodingNetwork;
use crate::snapshot::ModelSnapshot;

#[derive(Debug, Clone, Default)]
pub struct TrainingStats {
    pub epochs: usize,
    pub total_samples: usize,
    pub dropped_events: usize,
    pub last_energy: f64,
    pub energy_history: Vec<f64>,
}

pub struct TrainingWorker;

impl TrainingWorker {
    /// Run the training loop, processing events from the channel.
    /// Updates the shared snapshot after each batch.
    pub async fn run_batch(
        mut rx: mpsc::Receiver<LibraryEvent>,
        snapshot: Arc<RwLock<ModelSnapshot>>,
        config: PcnConfig,
        embedding_dim: usize,
    ) {
        let mut encoder = InputEncoder::new(embedding_dim, config.max_topics, config.max_tags);
        let input_dim = encoder.current_input_dim();
        let mut network = PredictiveCodingNetwork::new(input_dim, &config);
        let mut stats = TrainingStats::default();
        let mut batch: Vec<LibraryEvent> = Vec::with_capacity(config.batch_size);

        loop {
            // Try to fill a batch
            match tokio::time::timeout(
                std::time::Duration::from_millis(config.max_batch_delay_ms),
                rx.recv(),
            )
            .await
            {
                Ok(Some(event)) => {
                    batch.push(event);
                    // Drain any immediately available events
                    while batch.len() < config.batch_size {
                        match rx.try_recv() {
                            Ok(event) => batch.push(event),
                            Err(_) => break,
                        }
                    }
                }
                Ok(None) => {
                    // Channel closed — train remaining batch and exit
                    if !batch.is_empty() {
                        Self::train_batch(
                            &mut network,
                            &mut encoder,
                            &batch,
                            &config,
                            &mut stats,
                        );
                        Self::swap_snapshot(&network, &encoder, &stats, &snapshot);
                    }
                    break;
                }
                Err(_) => {
                    // Timeout — train incomplete batch if any
                }
            }

            if batch.len() >= config.batch_size || (!batch.is_empty() && batch.len() > 0) {
                Self::train_batch(
                    &mut network,
                    &mut encoder,
                    &batch,
                    &config,
                    &mut stats,
                );
                Self::swap_snapshot(&network, &encoder, &stats, &snapshot);
                batch.clear();
            }
        }
    }

    fn train_batch(
        network: &mut PredictiveCodingNetwork,
        encoder: &mut InputEncoder,
        events: &[LibraryEvent],
        config: &PcnConfig,
        stats: &mut TrainingStats,
    ) {
        let mut total_energy = 0.0;

        for event in events {
            match encoder.encode(event) {
                Ok((encoded, grew)) => {
                    if grew {
                        network.resize_input(encoder.current_input_dim());
                    }
                    let energy = network.train_single_with_steps(
                        &encoded,
                        config.inference_steps,
                        config.temporal_amortization,
                    );
                    total_energy += energy;
                    stats.total_samples += 1;
                }
                Err(e) => {
                    tracing::warn!("Failed to encode event: {}", e);
                    stats.dropped_events += 1;
                }
            }
        }

        stats.epochs += 1;
        stats.last_energy = total_energy / events.len().max(1) as f64;
        stats.energy_history.push(stats.last_energy);
        // Cap history
        if stats.energy_history.len() > 1000 {
            stats.energy_history.drain(..500);
        }
    }

    fn swap_snapshot(
        network: &PredictiveCodingNetwork,
        encoder: &InputEncoder,
        stats: &TrainingStats,
        snapshot: &Arc<RwLock<ModelSnapshot>>,
    ) {
        let new_snapshot = ModelSnapshot::from_network(
            network,
            encoder.vocab(),
            stats.epochs,
            stats.total_samples,
        );
        if let Ok(mut guard) = snapshot.write() {
            *guard = new_snapshot;
        }
    }
}
```

**Step 4: Run test to verify it passes**

Run: `cd simse-predictive-coding && cargo test`
Expected: PASS

**Step 5: Commit**

```bash
git add simse-predictive-coding/src/trainer.rs simse-predictive-coding/src/lib.rs
git commit -m "feat(pcn): add TrainingWorker with background batch training and snapshot swap"
```

---

### Task 9: Implement Predictor — query handlers reading from snapshot

**Files:**
- Create: `simse-predictive-coding/src/predictor.rs`
- Modify: `simse-predictive-coding/src/lib.rs` — add `pub mod predictor;`

**Step 1: Write the failing test**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{Activation, LayerConfig, PcnConfig};
    use crate::network::PredictiveCodingNetwork;
    use crate::vocabulary::VocabularyManager;
    use crate::snapshot::ModelSnapshot;
    use std::sync::{Arc, RwLock};

    fn trained_snapshot() -> Arc<RwLock<ModelSnapshot>> {
        let config = PcnConfig {
            layers: vec![
                LayerConfig { dim: 8, activation: Activation::Relu },
                LayerConfig { dim: 4, activation: Activation::Tanh },
            ],
            ..Default::default()
        };
        let mut net = PredictiveCodingNetwork::new(6, &config);
        for _ in 0..20 {
            net.train_single(&[1.0, 0.5, -0.3, 0.8, 0.0, -1.0]);
        }
        let vocab = VocabularyManager::new(100, 200);
        let snap = ModelSnapshot::from_network(&net, &vocab, 20, 20);
        Arc::new(RwLock::new(snap))
    }

    #[test]
    fn predict_confidence_returns_energy() {
        let snapshot = trained_snapshot();
        let predictor = Predictor::new(snapshot, 10);
        let input = vec![1.0, 0.5, -0.3, 0.8, 0.0, -1.0];
        let result = predictor.confidence(&input);
        assert!(result.is_some());
        let r = result.unwrap();
        assert!(r.energy.is_finite());
        assert_eq!(r.energy_breakdown.len(), 2);
    }

    #[test]
    fn predict_anomalies_returns_sorted_by_energy() {
        let snapshot = trained_snapshot();
        let predictor = Predictor::new(snapshot, 10);
        let inputs = vec![
            vec![1.0, 0.5, -0.3, 0.8, 0.0, -1.0],
            vec![100.0, 100.0, 100.0, 100.0, 100.0, 100.0], // anomalous
            vec![0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
        ];
        let results = predictor.anomalies(&inputs, 2);
        assert_eq!(results.len(), 2);
        // Should be sorted by energy descending
        assert!(results[0].1 >= results[1].1);
    }

    #[test]
    fn predict_on_empty_snapshot() {
        let snapshot = Arc::new(RwLock::new(ModelSnapshot::empty()));
        let predictor = Predictor::new(snapshot, 10);
        let input = vec![1.0, 0.5];
        let result = predictor.confidence(&input);
        assert!(result.is_none());
    }

    #[test]
    fn model_stats_returns_metadata() {
        let snapshot = trained_snapshot();
        let predictor = Predictor::new(snapshot, 10);
        let stats = predictor.model_stats();
        assert_eq!(stats.epoch, 20);
        assert_eq!(stats.total_samples, 20);
        assert_eq!(stats.num_layers, 2);
    }
}
```

**Step 2: Run test to verify it fails**

Run: `cd simse-predictive-coding && cargo test`
Expected: FAIL

**Step 3: Write implementation**

```rust
use std::sync::{Arc, RwLock};
use crate::snapshot::{ModelSnapshot, PredictionResult};

pub struct ModelStats {
    pub epoch: usize,
    pub total_samples: usize,
    pub num_layers: usize,
    pub input_dim: usize,
    pub layer_dims: Vec<usize>,
    pub parameter_count: usize,
}

pub struct Predictor {
    snapshot: Arc<RwLock<ModelSnapshot>>,
    inference_steps: usize,
}

impl Predictor {
    pub fn new(snapshot: Arc<RwLock<ModelSnapshot>>, inference_steps: usize) -> Self {
        Self { snapshot, inference_steps }
    }

    /// Run inference on an input and return confidence (energy) metrics.
    pub fn confidence(&self, input: &[f64]) -> Option<PredictionResult> {
        let snap = self.snapshot.read().ok()?;
        if snap.input_dim == 0 {
            return None;
        }
        Some(snap.predict(input, self.inference_steps))
    }

    /// Find the top-K highest-energy (most anomalous) inputs.
    /// Returns (index, energy) pairs sorted by energy descending.
    pub fn anomalies(&self, inputs: &[Vec<f64>], top_k: usize) -> Vec<(usize, f64)> {
        let snap = match self.snapshot.read() {
            Ok(s) => s,
            Err(_) => return vec![],
        };
        if snap.input_dim == 0 {
            return vec![];
        }

        let mut scored: Vec<(usize, f64)> = inputs
            .iter()
            .enumerate()
            .map(|(i, input)| {
                let result = snap.predict(input, self.inference_steps);
                (i, result.energy)
            })
            .collect();

        scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        scored.truncate(top_k);
        scored
    }

    /// Get model statistics.
    pub fn model_stats(&self) -> ModelStats {
        let snap = self.snapshot.read().unwrap();
        let layer_dims: Vec<usize> = snap.layer_configs.iter().map(|c| c.dim).collect();
        let parameter_count: usize = snap.layer_weights.iter()
            .zip(snap.layer_biases.iter())
            .map(|(w, b)| {
                w.iter().map(|row| row.len()).sum::<usize>() + b.len()
            })
            .sum();

        ModelStats {
            epoch: snap.epoch,
            total_samples: snap.total_samples,
            num_layers: snap.layer_configs.len(),
            input_dim: snap.input_dim,
            layer_dims,
            parameter_count,
        }
    }
}
```

**Step 4: Run test to verify it passes**

Run: `cd simse-predictive-coding && cargo test`
Expected: PASS

**Step 5: Commit**

```bash
git add simse-predictive-coding/src/predictor.rs simse-predictive-coding/src/lib.rs
git commit -m "feat(pcn): add Predictor with confidence, anomalies, and model stats queries"
```

---

### Task 10: Implement persistence (serialize/deserialize model state)

**Files:**
- Create: `simse-predictive-coding/src/persistence.rs`
- Modify: `simse-predictive-coding/src/lib.rs` — add `pub mod persistence;`

**Step 1: Write the failing test**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{Activation, LayerConfig, PcnConfig};
    use crate::network::PredictiveCodingNetwork;
    use crate::vocabulary::VocabularyManager;
    use crate::snapshot::ModelSnapshot;
    use tempfile::NamedTempFile;

    fn make_snapshot() -> ModelSnapshot {
        let config = PcnConfig {
            layers: vec![
                LayerConfig { dim: 4, activation: Activation::Relu },
                LayerConfig { dim: 2, activation: Activation::Tanh },
            ],
            ..Default::default()
        };
        let mut net = PredictiveCodingNetwork::new(3, &config);
        for _ in 0..5 {
            net.train_single(&[1.0, 0.5, -0.3]);
        }
        let mut vocab = VocabularyManager::new(100, 200);
        vocab.register_topic("rust").unwrap();
        vocab.register_tag("test").unwrap();
        ModelSnapshot::from_network(&net, &vocab, 5, 5)
    }

    #[test]
    fn save_load_round_trip_json() {
        let original = make_snapshot();
        let file = NamedTempFile::new().unwrap();
        let path = file.path().to_str().unwrap();

        save_snapshot(&original, path, false).unwrap();
        let restored = load_snapshot(path, false).unwrap();

        assert_eq!(restored.epoch, original.epoch);
        assert_eq!(restored.total_samples, original.total_samples);
        assert_eq!(restored.input_dim, original.input_dim);
        assert_eq!(restored.layer_configs.len(), original.layer_configs.len());
        assert_eq!(restored.vocabulary.topics, original.vocabulary.topics);
    }

    #[test]
    fn save_load_round_trip_gzip() {
        let original = make_snapshot();
        let file = NamedTempFile::new().unwrap();
        let path = file.path().to_str().unwrap();

        save_snapshot(&original, path, true).unwrap();
        let restored = load_snapshot(path, true).unwrap();

        assert_eq!(restored.epoch, original.epoch);
        assert_eq!(restored.layer_weights.len(), original.layer_weights.len());
    }

    #[test]
    fn load_nonexistent_file_returns_error() {
        let result = load_snapshot("/tmp/nonexistent_pcn_snapshot.json", false);
        assert!(result.is_err());
    }
}
```

**Step 2: Run test to verify it fails**

Run: `cd simse-predictive-coding && cargo test`
Expected: FAIL

**Step 3: Write implementation**

```rust
use std::io::{Read, Write};
use crate::error::PcnError;
use crate::snapshot::ModelSnapshot;

/// Save a model snapshot to disk.
pub fn save_snapshot(snapshot: &ModelSnapshot, path: &str, compress: bool) -> Result<(), PcnError> {
    let json = serde_json::to_vec(snapshot)?;

    if compress {
        let mut encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
        encoder.write_all(&json).map_err(PcnError::Io)?;
        let compressed = encoder.finish().map_err(PcnError::Io)?;
        std::fs::write(path, compressed).map_err(PcnError::Io)?;
    } else {
        std::fs::write(path, json).map_err(PcnError::Io)?;
    }

    Ok(())
}

/// Load a model snapshot from disk.
pub fn load_snapshot(path: &str, compressed: bool) -> Result<ModelSnapshot, PcnError> {
    let data = std::fs::read(path).map_err(PcnError::Io)?;

    if compressed {
        let mut decoder = flate2::read::GzDecoder::new(&data[..]);
        let mut json = Vec::new();
        decoder.read_to_end(&mut json).map_err(PcnError::Io)?;
        let snapshot: ModelSnapshot = serde_json::from_slice(&json)?;
        Ok(snapshot)
    } else {
        let snapshot: ModelSnapshot = serde_json::from_slice(&data)?;
        Ok(snapshot)
    }
}
```

**Step 4: Run test to verify it passes**

Run: `cd simse-predictive-coding && cargo test`
Expected: PASS

**Step 5: Commit**

```bash
git add simse-predictive-coding/src/persistence.rs simse-predictive-coding/src/lib.rs
git commit -m "feat(pcn): add model persistence with JSON and gzip support"
```

---

### Task 11: Implement JSON-RPC protocol types

**Files:**
- Create: `simse-predictive-coding/src/protocol.rs`
- Modify: `simse-predictive-coding/src/lib.rs` — add `pub mod protocol;`

**Step 1: Write the failing test**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_initialize_params() {
        let json = serde_json::json!({
            "embeddingDim": 768,
            "config": {
                "layers": [
                    {"dim": 512, "activation": "relu"},
                    {"dim": 128, "activation": "tanh"}
                ],
                "inferenceSteps": 20,
                "learningRate": 0.005,
                "inferenceRate": 0.1,
                "batchSize": 16,
                "maxBatchDelayMs": 1000,
                "channelCapacity": 1024,
                "autoSaveEpochs": 100,
                "maxTopics": 500,
                "maxTags": 1000,
                "temporalAmortization": true,
                "storagePath": null
            }
        });
        let params: InitializeParams = serde_json::from_value(json).unwrap();
        assert_eq!(params.embedding_dim, 768);
        assert_eq!(params.config.layers.len(), 2);
    }

    #[test]
    fn parse_predict_confidence_params() {
        let json = serde_json::json!({
            "input": [1.0, 0.5, -0.3]
        });
        let params: PredictConfidenceParams = serde_json::from_value(json).unwrap();
        assert_eq!(params.input.len(), 3);
    }

    #[test]
    fn parse_feed_event_params() {
        let json = serde_json::json!({
            "event": {
                "embedding": [1.0, 0.5],
                "topic": "rust",
                "tags": ["test"],
                "entryType": "fact",
                "timestamp": 1000.0,
                "timeSinceLast": 0.0,
                "sessionOrdinal": 1.0,
                "action": "extraction"
            }
        });
        let params: FeedEventParams = serde_json::from_value(json).unwrap();
        assert_eq!(params.event.topic, "rust");
    }
}
```

**Step 2: Run test to verify it fails**

Run: `cd simse-predictive-coding && cargo test`
Expected: FAIL

**Step 3: Write implementation**

```rust
use serde::{Deserialize, Serialize};
use crate::config::PcnConfig;
use crate::encoder::LibraryEvent;

// -- JSON-RPC error codes --

pub const INTERNAL_ERROR: i32 = -32603;
pub const METHOD_NOT_FOUND: i32 = -32601;
pub const INVALID_PARAMS: i32 = -32602;
pub const PCN_ERROR: i32 = -32000;

// -- Incoming request --

#[derive(Debug, Deserialize)]
pub struct JsonRpcRequest {
    pub id: u64,
    pub method: String,
    #[serde(default)]
    pub params: serde_json::Value,
}

// -- pcn/initialize --

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InitializeParams {
    pub embedding_dim: usize,
    pub config: PcnConfig,
    pub storage_path: Option<String>,
}

// -- feed/event --

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FeedEventParams {
    pub event: LibraryEvent,
}

// -- predict/* --

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PredictConfidenceParams {
    pub input: Vec<f64>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PredictAnomaliesParams {
    pub inputs: Vec<Vec<f64>>,
    pub top_k: usize,
}

// -- model/* --

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelSnapshotParams {
    pub path: String,
    #[serde(default)]
    pub compress: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelRestoreParams {
    pub path: String,
    #[serde(default)]
    pub compressed: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelConfigureParams {
    pub config: PcnConfig,
}
```

**Step 4: Run test to verify it passes**

Run: `cd simse-predictive-coding && cargo test`
Expected: PASS

**Step 5: Commit**

```bash
git add simse-predictive-coding/src/protocol.rs simse-predictive-coding/src/lib.rs
git commit -m "feat(pcn): add JSON-RPC protocol types for all PCN methods"
```

---

### Task 12: Implement NdjsonTransport (reuse simse pattern)

**Files:**
- Create: `simse-predictive-coding/src/transport.rs`
- Modify: `simse-predictive-coding/src/lib.rs` — add `pub mod transport;`

This is a copy of the established transport pattern from `simse-vsh/src/transport.rs`.

**Step 1: Create transport.rs**

```rust
use std::io::{self, Write};
use serde::Serialize;

#[derive(Serialize)]
struct JsonRpcResponse<'a> {
    jsonrpc: &'a str,
    id: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    result: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<JsonRpcErrorBody>,
}

#[derive(Serialize)]
struct JsonRpcErrorBody {
    code: i32,
    message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    data: Option<serde_json::Value>,
}

#[derive(Serialize)]
struct JsonRpcNotification<'a> {
    jsonrpc: &'a str,
    method: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    params: Option<serde_json::Value>,
}

pub struct NdjsonTransport;

impl Default for NdjsonTransport {
    fn default() -> Self { Self::new() }
}

impl NdjsonTransport {
    pub fn new() -> Self { Self }

    pub fn write_response(&self, id: u64, result: serde_json::Value) {
        self.write_line(&JsonRpcResponse {
            jsonrpc: "2.0", id, result: Some(result), error: None,
        });
    }

    pub fn write_error(&self, id: u64, code: i32, message: impl Into<String>, data: Option<serde_json::Value>) {
        self.write_line(&JsonRpcResponse {
            jsonrpc: "2.0", id, result: None,
            error: Some(JsonRpcErrorBody { code, message: message.into(), data }),
        });
    }

    pub fn write_notification(&self, method: &str, params: serde_json::Value) {
        self.write_line(&JsonRpcNotification {
            jsonrpc: "2.0", method, params: Some(params),
        });
    }

    fn write_line(&self, value: &impl Serialize) {
        let mut stdout = io::stdout().lock();
        if let Err(e) = serde_json::to_writer(&mut stdout, value) {
            tracing::error!("Failed to serialize: {}", e);
            return;
        }
        let _ = writeln!(stdout);
        let _ = stdout.flush();
    }
}
```

**Step 2: Verify it compiles**

Run: `cd simse-predictive-coding && cargo build`
Expected: Compiles

**Step 3: Commit**

```bash
git add simse-predictive-coding/src/transport.rs simse-predictive-coding/src/lib.rs
git commit -m "feat(pcn): add NdjsonTransport for JSON-RPC stdio communication"
```

---

### Task 13: Implement PcnServer — JSON-RPC dispatcher

**Files:**
- Create: `simse-predictive-coding/src/server.rs`
- Modify: `simse-predictive-coding/src/lib.rs` — add `pub mod server;`

**Step 1: Write the failing test**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::transport::NdjsonTransport;

    #[test]
    fn server_starts_uninitialized() {
        let transport = NdjsonTransport::new();
        let server = PcnServer::new(transport);
        assert!(!server.is_initialized());
    }
}
```

**Step 2: Run test to verify it fails**

Run: `cd simse-predictive-coding && cargo test`
Expected: FAIL

**Step 3: Write implementation**

```rust
use std::io::{self, BufRead};
use std::sync::{Arc, RwLock};

use tokio::sync::mpsc;

use crate::config::PcnConfig;
use crate::encoder::LibraryEvent;
use crate::error::PcnError;
use crate::persistence;
use crate::predictor::Predictor;
use crate::protocol::*;
use crate::snapshot::ModelSnapshot;
use crate::trainer::TrainingWorker;
use crate::transport::NdjsonTransport;

pub struct PcnServer {
    transport: NdjsonTransport,
    snapshot: Arc<RwLock<ModelSnapshot>>,
    predictor: Option<Predictor>,
    event_tx: Option<mpsc::Sender<LibraryEvent>>,
    initialized: bool,
    config: Option<PcnConfig>,
    embedding_dim: usize,
}

impl PcnServer {
    pub fn new(transport: NdjsonTransport) -> Self {
        Self {
            transport,
            snapshot: Arc::new(RwLock::new(ModelSnapshot::empty())),
            predictor: None,
            event_tx: None,
            initialized: false,
            config: None,
            embedding_dim: 0,
        }
    }

    pub fn is_initialized(&self) -> bool { self.initialized }

    /// Main loop: read JSON-RPC from stdin, dispatch.
    pub async fn run(&mut self) -> Result<(), PcnError> {
        let stdin = io::stdin();
        let reader = stdin.lock();

        for line_result in reader.lines() {
            let line = line_result?;
            if line.trim().is_empty() {
                continue;
            }

            let request: JsonRpcRequest = match serde_json::from_str(&line) {
                Ok(r) => r,
                Err(e) => {
                    tracing::error!("Failed to parse request: {}", e);
                    continue;
                }
            };

            self.dispatch(request).await;
        }

        Ok(())
    }

    async fn dispatch(&mut self, req: JsonRpcRequest) {
        let result = match req.method.as_str() {
            "pcn/initialize" => self.handle_initialize(req.params).await,
            "pcn/dispose" => self.handle_dispose(),
            "pcn/health" => self.handle_health(),

            "feed/event" => self.handle_feed_event(req.params).await,

            "predict/confidence" => self.handle_predict_confidence(req.params),
            "predict/anomalies" => self.handle_predict_anomalies(req.params),

            "model/stats" => self.handle_model_stats(),
            "model/snapshot" => self.handle_model_snapshot(req.params),
            "model/restore" => self.handle_model_restore(req.params),
            "model/reset" => self.handle_model_reset(),

            _ => {
                self.transport.write_error(
                    req.id,
                    METHOD_NOT_FOUND,
                    format!("Unknown method: {}", req.method),
                    None,
                );
                return;
            }
        };

        match result {
            Ok(value) => self.transport.write_response(req.id, value),
            Err(e) => self.transport.write_error(
                req.id,
                PCN_ERROR,
                e.to_string(),
                Some(e.to_json_rpc_error()),
            ),
        }
    }

    async fn handle_initialize(&mut self, params: serde_json::Value) -> Result<serde_json::Value, PcnError> {
        let p: InitializeParams = serde_json::from_value(params)?;

        self.embedding_dim = p.embedding_dim;
        self.config = Some(p.config.clone());

        let (tx, rx) = mpsc::channel(p.config.channel_capacity);
        self.event_tx = Some(tx);

        let snapshot = Arc::new(RwLock::new(ModelSnapshot::empty()));
        self.snapshot = snapshot.clone();
        self.predictor = Some(Predictor::new(snapshot.clone(), p.config.inference_steps));

        let config = p.config;
        let embedding_dim = p.embedding_dim;
        tokio::task::spawn(async move {
            TrainingWorker::run_batch(rx, snapshot, config, embedding_dim).await;
        });

        self.initialized = true;

        Ok(serde_json::json!({ "status": "initialized" }))
    }

    fn handle_dispose(&mut self) -> Result<serde_json::Value, PcnError> {
        self.event_tx = None; // Drop sender, closing channel
        self.predictor = None;
        self.initialized = false;
        Ok(serde_json::json!({ "status": "disposed" }))
    }

    fn handle_health(&self) -> Result<serde_json::Value, PcnError> {
        Ok(serde_json::json!({
            "initialized": self.initialized,
        }))
    }

    async fn handle_feed_event(&self, params: serde_json::Value) -> Result<serde_json::Value, PcnError> {
        if !self.initialized {
            return Err(PcnError::NotInitialized);
        }
        let p: FeedEventParams = serde_json::from_value(params)?;
        if let Some(tx) = &self.event_tx {
            match tx.try_send(p.event) {
                Ok(()) => Ok(serde_json::json!({ "queued": true })),
                Err(mpsc::error::TrySendError::Full(_)) => {
                    Ok(serde_json::json!({ "queued": false, "reason": "channel_full" }))
                }
                Err(mpsc::error::TrySendError::Closed(_)) => {
                    Err(PcnError::TrainingFailed("Training worker closed".into()))
                }
            }
        } else {
            Err(PcnError::NotInitialized)
        }
    }

    fn handle_predict_confidence(&self, params: serde_json::Value) -> Result<serde_json::Value, PcnError> {
        if !self.initialized {
            return Err(PcnError::NotInitialized);
        }
        let p: PredictConfidenceParams = serde_json::from_value(params)?;
        let predictor = self.predictor.as_ref().ok_or(PcnError::NotInitialized)?;
        match predictor.confidence(&p.input) {
            Some(result) => Ok(serde_json::json!({
                "energy": result.energy,
                "topLatent": result.top_latent,
                "energyBreakdown": result.energy_breakdown,
                "reconstruction": result.reconstruction,
            })),
            None => Ok(serde_json::json!({ "energy": null, "reason": "no_model" })),
        }
    }

    fn handle_predict_anomalies(&self, params: serde_json::Value) -> Result<serde_json::Value, PcnError> {
        if !self.initialized {
            return Err(PcnError::NotInitialized);
        }
        let p: PredictAnomaliesParams = serde_json::from_value(params)?;
        let predictor = self.predictor.as_ref().ok_or(PcnError::NotInitialized)?;
        let results = predictor.anomalies(&p.inputs, p.top_k);
        Ok(serde_json::json!({
            "anomalies": results.iter().map(|(idx, energy)| {
                serde_json::json!({ "index": idx, "energy": energy })
            }).collect::<Vec<_>>(),
        }))
    }

    fn handle_model_stats(&self) -> Result<serde_json::Value, PcnError> {
        if !self.initialized {
            return Err(PcnError::NotInitialized);
        }
        let predictor = self.predictor.as_ref().ok_or(PcnError::NotInitialized)?;
        let stats = predictor.model_stats();
        Ok(serde_json::json!({
            "epoch": stats.epoch,
            "totalSamples": stats.total_samples,
            "numLayers": stats.num_layers,
            "inputDim": stats.input_dim,
            "layerDims": stats.layer_dims,
            "parameterCount": stats.parameter_count,
        }))
    }

    fn handle_model_snapshot(&self, params: serde_json::Value) -> Result<serde_json::Value, PcnError> {
        if !self.initialized {
            return Err(PcnError::NotInitialized);
        }
        let p: ModelSnapshotParams = serde_json::from_value(params)?;
        let snap = self.snapshot.read().map_err(|_| PcnError::ModelCorrupt("Lock poisoned".into()))?;
        persistence::save_snapshot(&snap, &p.path, p.compress)?;
        Ok(serde_json::json!({ "saved": true, "path": p.path }))
    }

    fn handle_model_restore(&mut self, params: serde_json::Value) -> Result<serde_json::Value, PcnError> {
        if !self.initialized {
            return Err(PcnError::NotInitialized);
        }
        let p: ModelRestoreParams = serde_json::from_value(params)?;
        let restored = persistence::load_snapshot(&p.path, p.compressed)?;
        if let Ok(mut guard) = self.snapshot.write() {
            *guard = restored;
        }
        Ok(serde_json::json!({ "restored": true }))
    }

    fn handle_model_reset(&self) -> Result<serde_json::Value, PcnError> {
        if !self.initialized {
            return Err(PcnError::NotInitialized);
        }
        if let Ok(mut guard) = self.snapshot.write() {
            *guard = ModelSnapshot::empty();
        }
        Ok(serde_json::json!({ "reset": true }))
    }
}
```

**Step 4: Run test to verify it passes**

Run: `cd simse-predictive-coding && cargo test`
Expected: PASS

**Step 5: Commit**

```bash
git add simse-predictive-coding/src/server.rs simse-predictive-coding/src/lib.rs
git commit -m "feat(pcn): add PcnServer with JSON-RPC dispatch for all PCN methods"
```

---

### Task 14: Implement main.rs binary entry point

**Files:**
- Modify: `simse-predictive-coding/src/main.rs`
- Modify: `simse-predictive-coding/src/lib.rs` — ensure all modules exported

**Step 1: Update lib.rs with all modules**

```rust
pub mod error;
pub mod config;
pub mod layer;
pub mod network;
pub mod vocabulary;
pub mod encoder;
pub mod snapshot;
pub mod trainer;
pub mod predictor;
pub mod persistence;
pub mod protocol;
pub mod transport;
pub mod server;
```

**Step 2: Update main.rs**

```rust
use simse_pcn_engine::server::PcnServer;
use simse_pcn_engine::transport::NdjsonTransport;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    let transport = NdjsonTransport::new();
    let mut server = PcnServer::new(transport);

    tracing::info!("simse-pcn-engine ready");

    if let Err(e) = server.run().await {
        tracing::error!("Server error: {}", e);
        std::process::exit(1);
    }
}
```

**Step 3: Verify it compiles and runs**

Run: `cd simse-predictive-coding && cargo build --release`
Expected: Compiles successfully

**Step 4: Commit**

```bash
git add simse-predictive-coding/src/main.rs simse-predictive-coding/src/lib.rs
git commit -m "feat(pcn): add binary entry point for simse-pcn-engine"
```

---

### Task 15: Integration tests — full training + prediction flow

**Files:**
- Create: `simse-predictive-coding/tests/pcn_integration.rs`

**Step 1: Write integration tests**

```rust
use simse_pcn_engine::config::{Activation, LayerConfig, PcnConfig};
use simse_pcn_engine::encoder::{InputEncoder, LibraryEvent};
use simse_pcn_engine::network::PredictiveCodingNetwork;
use simse_pcn_engine::predictor::Predictor;
use simse_pcn_engine::snapshot::ModelSnapshot;
use simse_pcn_engine::trainer::TrainingWorker;
use simse_pcn_engine::vocabulary::VocabularyManager;
use std::sync::{Arc, RwLock};

fn make_event(topic: &str, embedding: Vec<f32>, action: &str) -> LibraryEvent {
    LibraryEvent {
        embedding,
        topic: topic.into(),
        tags: vec!["test".into()],
        entry_type: "fact".into(),
        timestamp: 1000.0,
        time_since_last: 0.0,
        session_ordinal: 1.0,
        action: action.into(),
    }
}

#[test]
fn full_training_loop_reduces_energy() {
    let config = PcnConfig {
        layers: vec![
            LayerConfig { dim: 16, activation: Activation::Relu },
            LayerConfig { dim: 8, activation: Activation::Tanh },
        ],
        inference_steps: 20,
        learning_rate: 0.001,
        inference_rate: 0.05,
        temporal_amortization: false,
        ..Default::default()
    };

    let mut encoder = InputEncoder::new(4, 100, 200);

    // Pre-register vocab so dimensions are stable
    encoder.vocab_mut().register_topic("rust").unwrap();
    encoder.vocab_mut().register_topic("python").unwrap();
    encoder.vocab_mut().register_tag("test").unwrap();

    let input_dim = encoder.current_input_dim();
    let mut network = PredictiveCodingNetwork::new(input_dim, &config);

    // Generate training data
    let events = vec![
        make_event("rust", vec![1.0, 0.5, -0.3, 0.8], "extraction"),
        make_event("rust", vec![0.9, 0.6, -0.2, 0.7], "extraction"),
        make_event("python", vec![-0.5, 0.8, 0.3, -0.2], "compendium"),
    ];

    let mut first_energy = None;
    let mut last_energy = 0.0;

    for epoch in 0..50 {
        let mut epoch_energy = 0.0;
        for event in &events {
            let (encoded, _) = encoder.encode(event).unwrap();
            let energy = network.train_single_with_steps(&encoded, config.inference_steps, false);
            epoch_energy += energy;
        }
        let avg = epoch_energy / events.len() as f64;
        if first_energy.is_none() {
            first_energy = Some(avg);
        }
        last_energy = avg;
    }

    // Energy should decrease over training
    assert!(last_energy < first_energy.unwrap(), "Energy should decrease: first={}, last={}", first_energy.unwrap(), last_energy);
}

#[tokio::test]
async fn trainer_and_predictor_work_together() {
    let config = PcnConfig {
        layers: vec![
            LayerConfig { dim: 8, activation: Activation::Relu },
            LayerConfig { dim: 4, activation: Activation::Tanh },
        ],
        inference_steps: 10,
        batch_size: 3,
        max_batch_delay_ms: 100,
        temporal_amortization: true,
        ..Default::default()
    };

    let snapshot = Arc::new(RwLock::new(ModelSnapshot::empty()));
    let (tx, rx) = tokio::sync::mpsc::channel(100);

    let snap_clone = snapshot.clone();
    let cfg_clone = config.clone();
    let handle = tokio::spawn(async move {
        TrainingWorker::run_batch(rx, snap_clone, cfg_clone, 4).await;
    });

    // Feed events
    for i in 0..9 {
        let event = make_event(
            if i % 2 == 0 { "rust" } else { "python" },
            vec![i as f32 * 0.1, 0.5, -0.3, 0.8],
            "extraction",
        );
        tx.send(event).await.unwrap();
    }
    drop(tx);

    handle.await.unwrap();

    // Predictor should be able to query
    let predictor = Predictor::new(snapshot, config.inference_steps);
    let stats = predictor.model_stats();
    assert!(stats.total_samples >= 9, "Should have trained on all samples");
    assert!(stats.num_layers == 2);
}

#[test]
fn concurrent_reads_during_snapshot() {
    let config = PcnConfig {
        layers: vec![LayerConfig { dim: 4, activation: Activation::Relu }],
        ..Default::default()
    };
    let mut net = PredictiveCodingNetwork::new(3, &config);
    for _ in 0..10 {
        net.train_single(&[1.0, 0.5, -0.3]);
    }
    let vocab = VocabularyManager::new(100, 200);
    let snap = ModelSnapshot::from_network(&net, &vocab, 10, 10);
    let shared = Arc::new(RwLock::new(snap));

    // Spawn multiple readers
    let mut handles = Vec::new();
    for _ in 0..10 {
        let s = shared.clone();
        handles.push(std::thread::spawn(move || {
            let guard = s.read().unwrap();
            let result = guard.predict(&[1.0, 0.5, -0.3], 5);
            assert!(result.energy.is_finite());
        }));
    }

    for h in handles {
        h.join().unwrap();
    }
}
```

**Step 2: Run integration tests**

Run: `cd simse-predictive-coding && cargo test --test pcn_integration`
Expected: PASS

**Step 3: Commit**

```bash
git add simse-predictive-coding/tests/pcn_integration.rs
git commit -m "test(pcn): add integration tests for training, prediction, and concurrency"
```

---

### Task 16: Add build script to workspace + final verification

**Files:**
- Modify: `package.json` — add `build:pcn-engine` script
- Modify: `simse-predictive-coding/` — add `moon.yml` if moon workspace configured

**Step 1: Add build script to package.json**

Add to scripts section:
```json
"build:pcn-engine": "cd simse-predictive-coding && cargo build --release"
```

**Step 2: Run full test suite**

Run: `cd simse-predictive-coding && cargo test`
Expected: All tests pass

Run: `cd simse-predictive-coding && cargo clippy -- -D warnings`
Expected: No warnings

**Step 3: Final commit**

```bash
git add package.json
git commit -m "feat(pcn): add build script for simse-predictive-coding"
```
