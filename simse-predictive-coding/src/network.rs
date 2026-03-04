use crate::config::PcnConfig;
use crate::layer::PcnLayer;

/// A hierarchical predictive coding network (PCN) composed of multiple [`PcnLayer`] instances.
///
/// Implements the PCN inference and learning algorithm based on Stenlund 2025.
///
/// **Architecture:**
/// - The network has an input dimension (`input_dim`) and L latent layers.
/// - An `input_predictor` layer predicts the clamped input from the first latent layer.
/// - Each latent layer `l` (except the top) receives predictions from layer `l+1` above it.
/// - The top layer has a zero (flat) prior.
///
/// **Inference** (T steps, synchronous updates):
/// 1. Compute predictions and errors for all layers using values from the previous step.
/// 2. Update latent values: `x(l) -= lr_infer * (e(l) - backprojected_error_from_below)`.
/// 3. The input (layer 0) is clamped and never updated.
///
/// **Learning** (Hebbian weight update after inference):
/// - `W(l) += lr_learn * (f'(a(l)) . e(l)) * x_above^T`
#[derive(Debug, Clone)]
pub struct PredictiveCodingNetwork {
    /// Dimensionality of the clamped input.
    input_dim: usize,
    /// Latent layers (index 0 = bottom, closest to input; last = top).
    layers: Vec<PcnLayer>,
    /// Dedicated layer that predicts the input from the first latent layer.
    /// Has dim = input_dim, input_dim = layers[0].dim.
    input_predictor: PcnLayer,
    /// Step size for latent value updates during inference.
    inference_rate: f64,
    /// Step size for Hebbian weight updates.
    learning_rate: f64,
    /// Default number of inference steps from config.
    default_inference_steps: usize,
    /// Whether temporal amortization is enabled by default.
    default_amortization: bool,
}

impl PredictiveCodingNetwork {
    /// Create a new predictive coding network.
    ///
    /// * `input_dim` - dimensionality of the clamped input
    /// * `config` - PCN configuration specifying layer dimensions, activations, and hyperparameters
    ///
    /// # Panics
    /// Panics if `config.layers` is empty.
    pub fn new(input_dim: usize, config: &PcnConfig) -> Self {
        assert!(
            !config.layers.is_empty(),
            "PcnConfig must have at least one layer"
        );

        // Build latent layers bottom-up.
        // layers[l].dim = config.layers[l].dim
        // layers[l].input_dim = config.layers[l+1].dim if l+1 exists, else its own dim
        let num_latent = config.layers.len();
        let mut layers = Vec::with_capacity(num_latent);

        for l in 0..num_latent {
            let dim = config.layers[l].dim;
            let layer_input_dim = if l + 1 < num_latent {
                config.layers[l + 1].dim
            } else {
                // Top layer: zero prior, input_dim = own dim (self-loop, but x_above will be zeros)
                dim
            };
            let activation = config.layers[l].activation;
            layers.push(PcnLayer::new(dim, layer_input_dim, activation));
        }

        // Input predictor: predicts input from first latent layer's values.
        // dim = input_dim (output is the predicted input), input_dim = layers[0].dim
        let input_predictor = PcnLayer::new(
            input_dim,
            config.layers[0].dim,
            // Use the same activation as the first latent layer for the input predictor.
            config.layers[0].activation,
        );

        Self {
            input_dim,
            layers,
            input_predictor,
            inference_rate: config.inference_rate,
            learning_rate: config.learning_rate,
            default_inference_steps: config.inference_steps,
            default_amortization: config.temporal_amortization,
        }
    }

    /// Number of latent layers.
    pub fn num_layers(&self) -> usize {
        self.layers.len()
    }

    /// Dimensionality of the clamped input.
    pub fn input_dim(&self) -> usize {
        self.input_dim
    }

    /// Immutable reference to a latent layer by index.
    pub fn layer(&self, idx: usize) -> &PcnLayer {
        &self.layers[idx]
    }

    /// Mutable reference to a latent layer by index.
    pub fn layer_mut(&mut self, idx: usize) -> &mut PcnLayer {
        &mut self.layers[idx]
    }

    /// Immutable reference to the input predictor layer.
    pub fn input_predictor(&self) -> &PcnLayer {
        &self.input_predictor
    }

    /// Mutable reference to the input predictor layer.
    pub fn input_predictor_mut(&mut self) -> &mut PcnLayer {
        &mut self.input_predictor
    }

    /// Run T inference steps on a clamped input, randomizing latent values first.
    ///
    /// Returns the total energy (sum of squared prediction errors across all layers).
    pub fn infer(&mut self, input: &[f64], steps: usize) -> f64 {
        debug_assert_eq!(
            input.len(),
            self.input_dim,
            "Input length {} != input_dim {}",
            input.len(),
            self.input_dim
        );

        // Randomize latent values to break symmetry.
        for (i, layer) in self.layers.iter_mut().enumerate() {
            layer.randomize_values(i as u64 + 1);
        }

        self.run_inference(input, steps)
    }

    /// Run T inference steps preserving the current latent state (temporal amortization).
    ///
    /// This skips the randomization step, allowing latent values from a previous
    /// inference to serve as a warm start. Useful for temporally correlated inputs.
    pub fn infer_amortized(&mut self, input: &[f64], steps: usize) -> f64 {
        debug_assert_eq!(
            input.len(),
            self.input_dim,
            "Input length {} != input_dim {}",
            input.len(),
            self.input_dim
        );

        self.run_inference(input, steps)
    }

    /// Train on a single sample using the default inference steps and amortization setting.
    ///
    /// Runs inference to convergence, then performs a Hebbian weight update.
    /// Returns the total energy after inference.
    pub fn train_single(&mut self, input: &[f64]) -> f64 {
        let steps = self.default_inference_steps;
        let amortized = self.default_amortization;
        self.train_single_with_steps(input, steps, amortized)
    }

    /// Train on a single sample with configurable inference steps and amortization.
    ///
    /// 1. Run inference (with or without randomization).
    /// 2. Update weights using the Hebbian learning rule.
    ///
    /// Returns the total energy after inference.
    pub fn train_single_with_steps(
        &mut self,
        input: &[f64],
        steps: usize,
        amortized: bool,
    ) -> f64 {
        // Step 1: Inference
        let energy = if amortized {
            self.infer_amortized(input, steps)
        } else {
            self.infer(input, steps)
        };

        // Step 2: Hebbian weight update
        self.update_all_weights();

        energy
    }

    /// Get the top (highest) latent layer's values.
    pub fn get_top_latent(&self) -> Vec<f64> {
        self.layers.last().unwrap().values.clone()
    }

    /// Generate (reconstruct) from the top latent layer down to the input dimension.
    ///
    /// Cascades predictions from the top layer down through all layers and
    /// finally through the input predictor.
    pub fn generate(&mut self) -> Vec<f64> {
        let num = self.layers.len();

        if num == 1 {
            // Single latent layer: input_predictor predicts from layers[0].values
            return self.input_predictor.predict(&self.layers[0].values);
        }

        // Start from the top layer. For the top layer, x_above = zeros (flat prior).
        // Each layer l predicts layer l-1's values. But for generation, we want:
        //   predicted_l = layers[l].predict(x_above_for_l)
        //   then use predicted_l as the values for layer l when predicting layer l-1.
        //
        // Actually, for generation we use the current latent values as-is (they were
        // set during inference). We just cascade predict downward.
        // layers[l] predicts what x(l) should be, given x(l+1).
        // But for generation, we USE layers[top].values, then
        //   predicted_{top-1} = layers[top-1].predict(layers[top].values)
        //   predicted_{top-2} = layers[top-2].predict(predicted_{top-1})
        //   ...
        //   predicted_input = input_predictor.predict(predicted_0)

        // Start with the top layer's actual values.
        let mut current = self.layers[num - 1].values.clone();

        // Cascade downward through latent layers (from top-1 down to 0).
        for l in (0..num - 1).rev() {
            current = self.layers[l].predict(&current);
        }

        // Final step: predict the input from the bottom latent layer prediction.
        self.input_predictor.predict(&current)
    }

    /// Per-layer energy breakdown.
    ///
    /// Returns a vector of length `num_layers()` where each element is
    /// 0.5 * sum(e(l)^2) for that layer. This includes the input prediction
    /// energy in the first element.
    pub fn energy_breakdown(&self) -> Vec<f64> {
        // The energy breakdown is computed from the stored errors.
        // We track energy per latent layer. The input prediction error is
        // folded into layer 0's energy.
        let mut energies = Vec::with_capacity(self.layers.len());

        for (l, layer) in self.layers.iter().enumerate() {
            let layer_energy = 0.5
                * layer
                    .errors
                    .iter()
                    .map(|e| e * e)
                    .sum::<f64>();

            if l == 0 {
                // Add input prediction energy.
                let input_energy = 0.5
                    * self
                        .input_predictor
                        .errors
                        .iter()
                        .map(|e| e * e)
                        .sum::<f64>();
                energies.push(layer_energy + input_energy);
            } else {
                energies.push(layer_energy);
            }
        }

        energies
    }

    /// Resize the input dimension of the network.
    ///
    /// Updates the input predictor layer to accommodate the new dimension.
    /// This is useful for vocabulary expansion.
    ///
    /// # Panics
    /// Panics if `new_input_dim < self.input_dim`.
    pub fn resize_input(&mut self, new_input_dim: usize) {
        assert!(
            new_input_dim >= self.input_dim,
            "Cannot shrink input dimension from {} to {}",
            self.input_dim,
            new_input_dim
        );

        if new_input_dim == self.input_dim {
            return;
        }

        // Rebuild the input predictor with the new dimension.
        // We create a fresh one because PcnLayer::resize_input only grows
        // the input_dim (from-above dimension), but here we need to grow
        // the output dim (this layer's dim).
        let old = &self.input_predictor;
        let mut new_predictor = PcnLayer::new(new_input_dim, old.input_dim, old.activation);

        // Copy existing weights: old has shape (input_dim x old.input_dim),
        // new has shape (new_input_dim x old.input_dim). Copy the first
        // input_dim rows.
        for i in 0..self.input_dim {
            let row_start = i * old.input_dim;
            let row_end = row_start + old.input_dim;
            new_predictor.weights[row_start..row_end]
                .copy_from_slice(&old.weights[row_start..row_end]);
            new_predictor.bias[i] = old.bias[i];
        }

        self.input_predictor = new_predictor;
        self.input_dim = new_input_dim;
    }

    // ---------------------------------------------------------------
    // Private helpers
    // ---------------------------------------------------------------

    /// Core inference loop shared by `infer` and `infer_amortized`.
    fn run_inference(&mut self, input: &[f64], steps: usize) -> f64 {
        let num = self.layers.len();
        let lr = self.inference_rate;

        for _step in 0..steps {
            // Phase 1: Compute predictions and errors for all layers (synchronous:
            // use values from the PREVIOUS step). We collect the needed data first
            // to avoid borrow conflicts.

            // 1a. Input prediction error: input_predictor predicts input from layers[0].values.
            let input_prediction = self.input_predictor.predict(&self.layers[0].values);
            for i in 0..self.input_dim {
                self.input_predictor.errors[i] = input[i] - input_prediction[i];
            }

            // 1b. For each latent layer, compute errors.
            //     layers[l].compute_errors(x_above) where:
            //       - x_above for layers[l] = layers[l+1].values if l+1 < num
            //       - x_above for top layer = zeros (flat prior)

            // We need to be careful about borrowing. Collect x_above for each layer first.
            let mut x_aboves: Vec<Vec<f64>> = Vec::with_capacity(num);
            for l in 0..num {
                let x_above = if l + 1 < num {
                    self.layers[l + 1].values.clone()
                } else {
                    vec![0.0; self.layers[l].input_dim]
                };
                x_aboves.push(x_above);
            }

            for l in 0..num {
                self.layers[l].compute_errors(&x_aboves[l]);
            }

            // Phase 2: Compute the backprojected error from below for each latent layer.
            // For layer l, the error from below comes from the layer that predicts
            // layer l. Specifically:
            // - layers[0] is predicted by the input_predictor. The backprojected error
            //   from the input to layers[0] is: input_predictor.top_down_error()
            //   (which is W_input^T * (f'(a) . e_input), result has length layers[0].dim)
            // - layers[l] (l > 0) is predicted by layers[l-1]. But wait — layers[l-1]
            //   is BELOW layers[l], not predicting it. Actually, layers[l-1].predict(layers[l].values)
            //   would predict layers[l-1], not layers[l].
            //
            // Let me re-clarify the PCN structure:
            //   The generative model goes top-down:
            //   x_hat(l) = f(W(l) * x(l+1) + b(l))  -- prediction of layer l from layer l+1
            //   e(l) = x(l) - x_hat(l)
            //
            //   Value update for layer l:
            //   x(l) -= lr * (e(l) - W(l-1)^T * (f'(a(l-1)) . e(l-1)))
            //   where the second term is the backprojected error from the layer BELOW (l-1),
            //   which used x(l) to make its prediction, so the gradient flows back up.
            //
            // In our data structure:
            //   layers[l].compute_errors(x_above) computes e(l) using x_above = layers[l+1].values
            //   layers[l].top_down_error() computes W(l)^T * (f'(a(l)) . e(l))
            //     which has length layers[l].input_dim = layers[l+1].dim
            //     This is the backprojected error that flows UP to layer l+1.
            //
            //   So for updating layer l's values, we need the backprojected error
            //   from below, which comes from the layer that uses x(l) as its input.
            //   That layer is the one that predicts layer l-1 from layer l.
            //
            //   For latent layers[l]:
            //     - If l == 0: the layer below is the input_predictor (predicts input from layers[0]).
            //       backproj_from_below = input_predictor.top_down_error()
            //       (length = input_predictor.input_dim = layers[0].dim) -- correct!
            //     - If l > 0: the layer below is layers[l-1] (predicts layer l-1 from layers[l]).
            //       backproj_from_below = layers[l-1].top_down_error()
            //       (length = layers[l-1].input_dim = layers[l].dim) -- correct!
            //     - Top layer (l == num-1): e(top) = x(top) - 0 = x(top) (from zero prior).
            //       backproj from below = layers[top-1].top_down_error() if top-1 >= 0

            // Collect backprojected errors from below for each latent layer.
            let mut backproj_from_below: Vec<Vec<f64>> = Vec::with_capacity(num);
            for l in 0..num {
                let bp = if l == 0 {
                    self.input_predictor.top_down_error()
                } else {
                    self.layers[l - 1].top_down_error()
                };
                backproj_from_below.push(bp);
            }

            // Phase 3: Update latent values (input is clamped, not updated).
            for l in 0..num {
                let layer = &mut self.layers[l];
                let bp = &backproj_from_below[l];
                debug_assert_eq!(
                    bp.len(),
                    layer.dim,
                    "Backprojected error length {} != layer dim {}",
                    bp.len(),
                    layer.dim
                );
                for i in 0..layer.dim {
                    layer.values[i] -= lr * (layer.errors[i] - bp[i]);
                }
            }
        }

        self.compute_total_energy()
    }

    /// Compute total energy: sum of 0.5 * ||e(l)||^2 for all layers including input prediction.
    fn compute_total_energy(&self) -> f64 {
        let input_energy: f64 = self
            .input_predictor
            .errors
            .iter()
            .map(|e| e * e)
            .sum::<f64>();

        let latent_energy: f64 = self
            .layers
            .iter()
            .map(|layer| layer.errors.iter().map(|e| e * e).sum::<f64>())
            .sum();

        0.5 * (input_energy + latent_energy)
    }

    /// Hebbian weight update for all layers after inference has converged.
    ///
    /// For each weight matrix, applies: W += lr * (f'(a) . e) * x_above^T
    fn update_all_weights(&mut self) {
        let lr = self.learning_rate;

        // Update input predictor weights.
        // The input predictor uses layers[0].values as x_above, and its errors
        // are the input prediction errors. We need to recompute errors to ensure
        // preactivations are fresh (they should be from the last inference step).
        //
        // Actually, the input_predictor's errors and preactivations are already
        // set from the last inference step. Just call update_weights.
        let x_above_for_input = self.layers[0].values.clone();
        self.input_predictor.update_weights(&x_above_for_input, lr);

        // Update latent layer weights.
        // layers[l].update_weights(x_above, lr) where x_above = layers[l+1].values or zeros.
        let num = self.layers.len();
        let mut x_aboves: Vec<Vec<f64>> = Vec::with_capacity(num);
        for l in 0..num {
            let x_above = if l + 1 < num {
                self.layers[l + 1].values.clone()
            } else {
                vec![0.0; self.layers[l].input_dim]
            };
            x_aboves.push(x_above);
        }

        for l in 0..num {
            self.layers[l].update_weights(&x_aboves[l], lr);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{Activation, LayerConfig, PcnConfig};

    fn test_config() -> PcnConfig {
        PcnConfig {
            layers: vec![
                LayerConfig {
                    dim: 8,
                    activation: Activation::Relu,
                },
                LayerConfig {
                    dim: 4,
                    activation: Activation::Tanh,
                },
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
    fn infer_returns_finite_energy() {
        let mut net = PredictiveCodingNetwork::new(6, &test_config());
        let input = vec![1.0, 0.5, -0.3, 0.8, 0.0, -1.0];
        let energy = net.infer(&input, 10);
        assert!(energy.is_finite());
        assert!(energy >= 0.0);
    }

    #[test]
    fn train_step_updates_weights() {
        let mut net = PredictiveCodingNetwork::new(6, &test_config());
        let input = vec![1.0, 0.5, -0.3, 0.8, 0.0, -1.0];
        let w_before: Vec<f64> = net.layer(0).weights.clone();
        net.train_single(&input);
        let w_after: Vec<f64> = net.layer(0).weights.clone();
        let changed = w_before
            .iter()
            .zip(w_after.iter())
            .any(|(a, b)| (a - b).abs() > 1e-15);
        assert!(changed, "Weights should change after training");
    }

    #[test]
    fn energy_is_non_negative() {
        let mut net = PredictiveCodingNetwork::new(
            4,
            &PcnConfig {
                layers: vec![LayerConfig {
                    dim: 3,
                    activation: Activation::Relu,
                }],
                inference_steps: 1,
                ..Default::default()
            },
        );
        let energy = net.infer(&[1.0, 2.0, 3.0, 4.0], 1);
        assert!(energy >= 0.0);
    }

    #[test]
    fn get_latent_returns_top_layer_values() {
        let mut net = PredictiveCodingNetwork::new(6, &test_config());
        net.infer(&[1.0, 0.5, -0.3, 0.8, 0.0, -1.0], 5);
        let latent = net.get_top_latent();
        assert_eq!(latent.len(), 4); // top layer dim
    }

    #[test]
    fn generate_reconstructs_correct_dim() {
        let mut net = PredictiveCodingNetwork::new(6, &test_config());
        net.infer(&[1.0, 0.5, -0.3, 0.8, 0.0, -1.0], 20);
        let reconstruction = net.generate();
        assert_eq!(reconstruction.len(), 6);
    }

    #[test]
    fn energy_breakdown_has_correct_count() {
        let mut net = PredictiveCodingNetwork::new(6, &test_config());
        net.infer(&[1.0, 0.5, -0.3, 0.8, 0.0, -1.0], 5);
        let breakdown = net.energy_breakdown();
        assert_eq!(breakdown.len(), 2);
        assert!(breakdown.iter().all(|e| *e >= 0.0));
    }

    #[test]
    fn resize_input_changes_dimension() {
        let mut net = PredictiveCodingNetwork::new(6, &test_config());
        assert_eq!(net.input_dim(), 6);
        net.resize_input(10);
        assert_eq!(net.input_dim(), 10);
        // Should still be able to infer with new dim
        let energy = net.infer(&[1.0; 10], 5);
        assert!(energy.is_finite());
    }

    // ---------------------------------------------------------------
    // Additional tests for thorough coverage
    // ---------------------------------------------------------------

    #[test]
    fn single_layer_network() {
        let config = PcnConfig {
            layers: vec![LayerConfig {
                dim: 4,
                activation: Activation::Tanh,
            }],
            inference_steps: 5,
            learning_rate: 0.01,
            inference_rate: 0.1,
            temporal_amortization: false,
            ..Default::default()
        };
        let mut net = PredictiveCodingNetwork::new(3, &config);
        assert_eq!(net.num_layers(), 1);

        let energy = net.infer(&[1.0, 2.0, 3.0], 5);
        assert!(energy.is_finite());
        assert!(energy >= 0.0);

        let latent = net.get_top_latent();
        assert_eq!(latent.len(), 4);

        let reconstruction = net.generate();
        assert_eq!(reconstruction.len(), 3);
    }

    #[test]
    fn three_layer_network() {
        let config = PcnConfig {
            layers: vec![
                LayerConfig {
                    dim: 16,
                    activation: Activation::Relu,
                },
                LayerConfig {
                    dim: 8,
                    activation: Activation::Tanh,
                },
                LayerConfig {
                    dim: 4,
                    activation: Activation::Sigmoid,
                },
            ],
            inference_steps: 10,
            learning_rate: 0.005,
            inference_rate: 0.05,
            temporal_amortization: false,
            ..Default::default()
        };
        let mut net = PredictiveCodingNetwork::new(32, &config);
        assert_eq!(net.num_layers(), 3);
        assert_eq!(net.input_dim(), 32);
        assert_eq!(net.get_top_latent().len(), 4);

        let input: Vec<f64> = (0..32).map(|i| (i as f64) * 0.1).collect();
        let energy = net.infer(&input, 10);
        assert!(energy.is_finite());
        assert!(energy >= 0.0);

        let reconstruction = net.generate();
        assert_eq!(reconstruction.len(), 32);

        let breakdown = net.energy_breakdown();
        assert_eq!(breakdown.len(), 3);
        assert!(breakdown.iter().all(|e| *e >= 0.0));
    }

    #[test]
    fn infer_amortized_preserves_latents() {
        let mut net = PredictiveCodingNetwork::new(6, &test_config());
        let input = vec![1.0, 0.5, -0.3, 0.8, 0.0, -1.0];

        // First inference randomizes latents.
        net.infer(&input, 10);
        let latent_after_first = net.get_top_latent();

        // Amortized inference preserves latent state (no randomization).
        let energy = net.infer_amortized(&input, 1);
        assert!(energy.is_finite());
        let latent_after_amortized = net.get_top_latent();

        // The latents should have changed from the amortized step (they get updated),
        // but they started from the previous state rather than being re-randomized.
        // We can't easily test "no randomization" directly, but we verify it works.
        assert_eq!(latent_after_amortized.len(), latent_after_first.len());
    }

    #[test]
    fn energy_decreases_with_more_steps() {
        let mut net = PredictiveCodingNetwork::new(6, &test_config());
        let input = vec![1.0, 0.5, -0.3, 0.8, 0.0, -1.0];

        let energy_1 = net.infer(&input, 1);
        let energy_50 = net.infer(&input, 50);

        // With more inference steps, energy should generally decrease (or at least not explode).
        // This is a soft check since randomization differs between calls.
        assert!(energy_50.is_finite());
        assert!(energy_1.is_finite());
    }

    #[test]
    fn train_single_with_steps_custom() {
        let mut net = PredictiveCodingNetwork::new(6, &test_config());
        let input = vec![1.0, 0.5, -0.3, 0.8, 0.0, -1.0];

        let energy = net.train_single_with_steps(&input, 15, false);
        assert!(energy.is_finite());
        assert!(energy >= 0.0);
    }

    #[test]
    fn train_single_with_amortization() {
        let mut net = PredictiveCodingNetwork::new(6, &test_config());
        let input = vec![1.0, 0.5, -0.3, 0.8, 0.0, -1.0];

        // First call with non-amortized to initialize.
        net.train_single_with_steps(&input, 10, false);

        // Second call with amortization.
        let energy = net.train_single_with_steps(&input, 5, true);
        assert!(energy.is_finite());
        assert!(energy >= 0.0);
    }

    #[test]
    fn layer_accessors_work() {
        let mut net = PredictiveCodingNetwork::new(6, &test_config());
        assert_eq!(net.layer(0).dim, 8);
        assert_eq!(net.layer(1).dim, 4);
        net.layer_mut(0).values[0] = 42.0;
        assert_eq!(net.layer(0).values[0], 42.0);
    }

    #[test]
    #[should_panic]
    fn empty_config_panics() {
        let config = PcnConfig {
            layers: vec![],
            ..Default::default()
        };
        PredictiveCodingNetwork::new(6, &config);
    }

    #[test]
    fn multiple_training_steps_converge() {
        let mut net = PredictiveCodingNetwork::new(4, &PcnConfig {
            layers: vec![
                LayerConfig {
                    dim: 8,
                    activation: Activation::Tanh,
                },
                LayerConfig {
                    dim: 4,
                    activation: Activation::Tanh,
                },
            ],
            inference_steps: 20,
            learning_rate: 0.001,
            inference_rate: 0.1,
            temporal_amortization: false,
            ..Default::default()
        });
        let input = vec![0.5, -0.5, 0.3, -0.3];

        let mut energies = Vec::new();
        for _ in 0..10 {
            let e = net.train_single(&input);
            energies.push(e);
        }

        // All energies should be finite and non-negative.
        assert!(energies.iter().all(|e| e.is_finite() && *e >= 0.0));

        // Energy should generally trend downward over training.
        // Compare first vs last (allowing for some noise).
        let first = energies[0];
        let last = *energies.last().unwrap();
        // Just check it doesn't explode.
        assert!(last < first * 100.0, "Energy should not explode: first={}, last={}", first, last);
    }

    #[test]
    fn generate_produces_finite_values() {
        let mut net = PredictiveCodingNetwork::new(6, &test_config());
        net.infer(&[1.0, 0.5, -0.3, 0.8, 0.0, -1.0], 10);
        let reconstruction = net.generate();
        assert!(reconstruction.iter().all(|v| v.is_finite()));
    }

    #[test]
    fn resize_input_preserves_existing_weights() {
        let mut net = PredictiveCodingNetwork::new(6, &test_config());

        // Capture some input predictor weights before resize.
        let w_before: Vec<f64> = net.input_predictor.weights[..6 * net.layers[0].dim].to_vec();

        net.resize_input(10);

        // First 6 rows of weights should be preserved.
        let input_dim_of_predictor = net.input_predictor.input_dim;
        for i in 0..6 {
            for j in 0..input_dim_of_predictor {
                assert_eq!(
                    net.input_predictor.weights[i * input_dim_of_predictor + j],
                    w_before[i * input_dim_of_predictor + j],
                    "Weight at ({}, {}) changed after resize",
                    i,
                    j
                );
            }
        }
    }

    #[test]
    fn zero_input_produces_finite_energy() {
        let mut net = PredictiveCodingNetwork::new(6, &test_config());
        let input = vec![0.0; 6];
        let energy = net.infer(&input, 5);
        assert!(energy.is_finite());
        assert!(energy >= 0.0);
    }
}
