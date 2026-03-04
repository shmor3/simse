use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};

use crate::config::Activation;

/// A single layer in a predictive coding network.
///
/// Holds value nodes (latent variables), error nodes, generative weights,
/// bias, and preactivations. This is the lowest-level building block that
/// the `PredictiveCodingNetwork` composes into a hierarchy.
///
/// **Notation** (layer index `l`):
/// - `values` = x(l), the latent state at this layer
/// - `errors` = e(l) = x(l) - x_hat(l)
/// - `weights` = W(l), generative weights mapping from the layer above
/// - `bias` = b(l)
/// - `preactivations` = a(l) = W * x_above + b (before activation)
/// - `x_hat(l) = f(a(l))` = prediction of this layer's values
#[derive(Debug, Clone)]
pub struct PcnLayer {
    /// Dimensionality of this layer (number of value/error nodes).
    pub dim: usize,
    /// Dimensionality of the input from the layer above.
    pub input_dim: usize,
    /// Activation function applied to preactivations.
    pub activation: Activation,
    /// Latent values x(l), length = dim.
    pub values: Vec<f64>,
    /// Prediction errors e(l) = x(l) - predict(x_above), length = dim.
    pub errors: Vec<f64>,
    /// Generative weights W(l), shape dim x input_dim (row-major).
    /// `weights[i * input_dim + j]` = W_{i,j}.
    pub weights: Vec<f64>,
    /// Bias vector b(l), length = dim.
    pub bias: Vec<f64>,
    /// Preactivations a(l) = W * x_above + b, length = dim.
    /// Stored after each call to `predict` so that `derivative(a)` is available
    /// for error backpropagation and weight updates.
    pub preactivations: Vec<f64>,
}

impl PcnLayer {
    /// Create a new layer with Xavier-initialized weights and zero bias.
    ///
    /// * `dim` - number of nodes in this layer
    /// * `input_dim` - number of nodes in the layer above (input to generative model)
    /// * `activation` - nonlinearity applied after the linear transform
    pub fn new(dim: usize, input_dim: usize, activation: Activation) -> Self {
        let mut rng = StdRng::from_entropy();
        Self::new_with_rng(dim, input_dim, activation, &mut rng)
    }

    /// Create a new layer with a caller-supplied RNG (useful for deterministic tests).
    fn new_with_rng(
        dim: usize,
        input_dim: usize,
        activation: Activation,
        rng: &mut impl Rng,
    ) -> Self {
        // Xavier initialization: W ~ U(-limit, +limit) where limit = sqrt(6 / (fan_in + fan_out))
        let limit = (6.0 / (input_dim as f64 + dim as f64)).sqrt();
        let weights: Vec<f64> = (0..dim * input_dim)
            .map(|_| rng.gen_range(-limit..limit))
            .collect();

        Self {
            dim,
            input_dim,
            activation,
            values: vec![0.0; dim],
            errors: vec![0.0; dim],
            weights,
            bias: vec![0.0; dim],
            preactivations: vec![0.0; dim],
        }
    }

    /// Compute the prediction x_hat(l) = f(W * x_above + b).
    ///
    /// Also stores the preactivations a(l) = W * x_above + b for later use
    /// in derivative computations.
    ///
    /// * `x_above` - values from the layer above, length must equal `input_dim`
    ///
    /// Returns the prediction vector of length `dim`.
    pub fn predict(&mut self, x_above: &[f64]) -> Vec<f64> {
        debug_assert_eq!(
            x_above.len(),
            self.input_dim,
            "x_above length {} != input_dim {}",
            x_above.len(),
            self.input_dim
        );

        // a = W * x_above + b
        for i in 0..self.dim {
            let mut sum = self.bias[i];
            let row_start = i * self.input_dim;
            for j in 0..self.input_dim {
                sum += self.weights[row_start + j] * x_above[j];
            }
            self.preactivations[i] = sum;
        }

        // x_hat = f(a)
        self.preactivations
            .iter()
            .map(|&a| self.activation.apply(a))
            .collect()
    }

    /// Compute prediction errors e(l) = x(l) - predict(x_above).
    ///
    /// Updates `self.errors` in place and returns a reference to the error vector.
    ///
    /// * `x_above` - values from the layer above
    pub fn compute_errors(&mut self, x_above: &[f64]) -> &[f64] {
        let prediction = self.predict(x_above);
        for i in 0..self.dim {
            self.errors[i] = self.values[i] - prediction[i];
        }
        &self.errors
    }

    /// Compute the top-down error signal W^T * (f'(a) . e).
    ///
    /// This is the gradient contribution that this layer sends to the layer
    /// above during inference (value updates). The result has length `input_dim`.
    ///
    /// Must be called after `compute_errors` (or after `predict` has set
    /// `preactivations` and `errors` have been set).
    pub fn top_down_error(&self) -> Vec<f64> {
        // modulated = f'(a) . e  (element-wise)
        let modulated: Vec<f64> = self
            .preactivations
            .iter()
            .zip(self.errors.iter())
            .map(|(&a, &e)| self.activation.derivative(a) * e)
            .collect();

        // W^T * modulated
        let mut result = vec![0.0; self.input_dim];
        for i in 0..self.dim {
            let row_start = i * self.input_dim;
            let m = modulated[i];
            for j in 0..self.input_dim {
                result[j] += self.weights[row_start + j] * m;
            }
        }
        result
    }

    /// Hebbian weight update: W += lr * (f'(a) . e) * x_above^T
    ///
    /// Also updates the bias: b += lr * (f'(a) . e)
    ///
    /// * `x_above` - values from the layer above
    /// * `lr` - learning rate
    pub fn update_weights(&mut self, x_above: &[f64], lr: f64) {
        debug_assert_eq!(
            x_above.len(),
            self.input_dim,
            "x_above length {} != input_dim {}",
            x_above.len(),
            self.input_dim
        );

        for i in 0..self.dim {
            let grad = self.activation.derivative(self.preactivations[i]) * self.errors[i];
            let scaled = lr * grad;
            let row_start = i * self.input_dim;
            for j in 0..self.input_dim {
                self.weights[row_start + j] += scaled * x_above[j];
            }
            self.bias[i] += scaled;
        }
    }

    /// Initialize latent values with small random perturbations.
    ///
    /// Values are drawn from U(-0.1, 0.1). This is used at the start of
    /// inference to break symmetry.
    ///
    /// * `seed` - deterministic seed for reproducibility
    pub fn randomize_values(&mut self, seed: u64) {
        let mut rng = StdRng::seed_from_u64(seed);
        for v in self.values.iter_mut() {
            *v = rng.gen_range(-0.1..0.1);
        }
    }

    /// Grow the input dimension to accommodate vocabulary expansion.
    ///
    /// New weight columns are Xavier-initialized. Existing weights are preserved.
    ///
    /// * `new_input_dim` - the new (larger) input dimension
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

        let mut rng = StdRng::from_entropy();
        let limit = (6.0 / (new_input_dim as f64 + self.dim as f64)).sqrt();

        let mut new_weights = vec![0.0; self.dim * new_input_dim];
        for i in 0..self.dim {
            // Copy existing weights for this row
            let old_start = i * self.input_dim;
            let new_start = i * new_input_dim;
            new_weights[new_start..new_start + self.input_dim]
                .copy_from_slice(&self.weights[old_start..old_start + self.input_dim]);

            // Xavier-init new columns
            for j in self.input_dim..new_input_dim {
                new_weights[new_start + j] = rng.gen_range(-limit..limit);
            }
        }

        self.weights = new_weights;
        self.input_dim = new_input_dim;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper: create a layer with a deterministic seed for reproducible tests.
    fn make_layer(dim: usize, input_dim: usize, activation: Activation) -> PcnLayer {
        let mut rng = StdRng::seed_from_u64(42);
        PcnLayer::new_with_rng(dim, input_dim, activation, &mut rng)
    }

    // ---------------------------------------------------------------
    // Dimension tests
    // ---------------------------------------------------------------

    #[test]
    fn new_layer_has_correct_dimensions() {
        let layer = make_layer(8, 4, Activation::Relu);
        assert_eq!(layer.dim, 8);
        assert_eq!(layer.input_dim, 4);
        assert_eq!(layer.values.len(), 8);
        assert_eq!(layer.errors.len(), 8);
        assert_eq!(layer.weights.len(), 8 * 4);
        assert_eq!(layer.bias.len(), 8);
        assert_eq!(layer.preactivations.len(), 8);
    }

    #[test]
    fn values_and_errors_initialized_to_zero() {
        let layer = make_layer(5, 3, Activation::Tanh);
        assert!(layer.values.iter().all(|&v| v == 0.0));
        assert!(layer.errors.iter().all(|&e| e == 0.0));
    }

    #[test]
    fn bias_initialized_to_zero() {
        let layer = make_layer(5, 3, Activation::Sigmoid);
        assert!(layer.bias.iter().all(|&b| b == 0.0));
    }

    #[test]
    fn weights_are_xavier_bounded() {
        let dim = 16;
        let input_dim = 8;
        let layer = make_layer(dim, input_dim, Activation::Relu);
        let limit = (6.0 / (input_dim as f64 + dim as f64)).sqrt();
        for &w in &layer.weights {
            assert!(
                w.abs() <= limit + 1e-12,
                "Weight {} exceeds Xavier bound {}",
                w,
                limit
            );
        }
    }

    // ---------------------------------------------------------------
    // predict tests
    // ---------------------------------------------------------------

    #[test]
    fn predict_with_identity_weights_relu() {
        // With identity weights (W=I), bias=0, and ReLU:
        // predict(x) = ReLU(I * x + 0) = ReLU(x)
        let mut layer = make_layer(3, 3, Activation::Relu);

        // Set weights to identity
        layer.weights = vec![
            1.0, 0.0, 0.0, //
            0.0, 1.0, 0.0, //
            0.0, 0.0, 1.0, //
        ];

        let x = vec![1.0, -2.0, 3.0];
        let pred = layer.predict(&x);
        assert_eq!(pred, vec![1.0, 0.0, 3.0]); // ReLU clips -2 to 0
    }

    #[test]
    fn predict_with_identity_weights_tanh() {
        let mut layer = make_layer(2, 2, Activation::Tanh);
        layer.weights = vec![1.0, 0.0, 0.0, 1.0];

        let x = vec![0.0, 1.0];
        let pred = layer.predict(&x);
        assert!((pred[0] - 0.0_f64.tanh()).abs() < 1e-12);
        assert!((pred[1] - 1.0_f64.tanh()).abs() < 1e-12);
    }

    #[test]
    fn predict_incorporates_bias() {
        let mut layer = make_layer(2, 2, Activation::Relu);
        layer.weights = vec![1.0, 0.0, 0.0, 1.0];
        layer.bias = vec![10.0, -5.0];

        let x = vec![1.0, 2.0];
        let pred = layer.predict(&x);
        // a = [1*1 + 0*2 + 10, 0*1 + 1*2 - 5] = [11, -3]
        // ReLU([11, -3]) = [11, 0]
        assert!((pred[0] - 11.0).abs() < 1e-12);
        assert!((pred[1] - 0.0).abs() < 1e-12);
    }

    #[test]
    fn predict_output_length_matches_dim() {
        let mut layer = make_layer(5, 3, Activation::Sigmoid);
        let x = vec![1.0, 2.0, 3.0];
        let pred = layer.predict(&x);
        assert_eq!(pred.len(), 5);
    }

    // ---------------------------------------------------------------
    // preactivations stored correctly
    // ---------------------------------------------------------------

    #[test]
    fn preactivations_stored_after_predict() {
        let mut layer = make_layer(3, 2, Activation::Relu);
        layer.weights = vec![
            1.0, 2.0, //
            3.0, 4.0, //
            5.0, 6.0, //
        ];
        layer.bias = vec![0.1, 0.2, 0.3];

        let x = vec![1.0, 1.0];
        layer.predict(&x);

        // a[0] = 1*1 + 2*1 + 0.1 = 3.1
        // a[1] = 3*1 + 4*1 + 0.2 = 7.2
        // a[2] = 5*1 + 6*1 + 0.3 = 11.3
        assert!((layer.preactivations[0] - 3.1).abs() < 1e-12);
        assert!((layer.preactivations[1] - 7.2).abs() < 1e-12);
        assert!((layer.preactivations[2] - 11.3).abs() < 1e-12);
    }

    // ---------------------------------------------------------------
    // activation application
    // ---------------------------------------------------------------

    #[test]
    fn activation_sigmoid_applied_correctly() {
        let mut layer = make_layer(1, 1, Activation::Sigmoid);
        layer.weights = vec![1.0];
        layer.bias = vec![0.0];

        let x = vec![0.0];
        let pred = layer.predict(&x);
        // sigmoid(0) = 0.5
        assert!((pred[0] - 0.5).abs() < 1e-12);
    }

    #[test]
    fn activation_tanh_applied_correctly() {
        let mut layer = make_layer(1, 1, Activation::Tanh);
        layer.weights = vec![1.0];
        layer.bias = vec![0.0];

        let x = vec![0.5];
        let pred = layer.predict(&x);
        assert!((pred[0] - 0.5_f64.tanh()).abs() < 1e-12);
    }

    // ---------------------------------------------------------------
    // compute_errors
    // ---------------------------------------------------------------

    #[test]
    fn error_equals_value_minus_prediction() {
        let mut layer = make_layer(3, 3, Activation::Relu);
        layer.weights = vec![
            1.0, 0.0, 0.0, //
            0.0, 1.0, 0.0, //
            0.0, 0.0, 1.0, //
        ];

        // Set values to known state
        layer.values = vec![2.0, 3.0, 4.0];

        let x_above = vec![1.0, 1.0, 1.0];
        // predict = ReLU(I * [1,1,1] + 0) = [1,1,1]
        // error = [2,3,4] - [1,1,1] = [1,2,3]
        let errors = layer.compute_errors(&x_above);
        assert!((errors[0] - 1.0).abs() < 1e-12);
        assert!((errors[1] - 2.0).abs() < 1e-12);
        assert!((errors[2] - 3.0).abs() < 1e-12);
    }

    #[test]
    fn error_is_zero_when_prediction_matches_values() {
        let mut layer = make_layer(2, 2, Activation::Relu);
        layer.weights = vec![1.0, 0.0, 0.0, 1.0];

        let x_above = vec![5.0, 7.0];
        // predict = ReLU([5, 7]) = [5, 7]
        layer.values = vec![5.0, 7.0];
        let errors = layer.compute_errors(&x_above);
        assert!(errors.iter().all(|&e| e.abs() < 1e-12));
    }

    #[test]
    fn compute_errors_updates_errors_field() {
        let mut layer = make_layer(2, 2, Activation::Relu);
        layer.weights = vec![1.0, 0.0, 0.0, 1.0];
        layer.values = vec![10.0, 20.0];

        let x_above = vec![3.0, 4.0];
        layer.compute_errors(&x_above);

        // Verify the field was updated (not just the return value)
        assert!((layer.errors[0] - 7.0).abs() < 1e-12);
        assert!((layer.errors[1] - 16.0).abs() < 1e-12);
    }

    // ---------------------------------------------------------------
    // top_down_error
    // ---------------------------------------------------------------

    #[test]
    fn top_down_error_has_correct_length() {
        let mut layer = make_layer(4, 6, Activation::Relu);
        layer.values = vec![1.0; 4];
        let x_above = vec![0.0; 6];
        layer.compute_errors(&x_above);
        let td = layer.top_down_error();
        assert_eq!(td.len(), 6);
    }

    #[test]
    fn top_down_error_identity_relu() {
        // With identity weights, ReLU, and preactivations > 0 (so f'(a) = 1):
        // top_down = W^T * (1 . e) = I^T * e = e
        let mut layer = make_layer(3, 3, Activation::Relu);
        layer.weights = vec![
            1.0, 0.0, 0.0, //
            0.0, 1.0, 0.0, //
            0.0, 0.0, 1.0, //
        ];

        // Force positive preactivations so f'(a)=1 for ReLU
        layer.preactivations = vec![1.0, 2.0, 3.0];
        layer.errors = vec![0.5, 1.5, 2.5];

        let td = layer.top_down_error();
        assert!((td[0] - 0.5).abs() < 1e-12);
        assert!((td[1] - 1.5).abs() < 1e-12);
        assert!((td[2] - 2.5).abs() < 1e-12);
    }

    #[test]
    fn top_down_error_relu_clips_negative_preactivations() {
        // When preactivation <= 0, ReLU derivative = 0, so that error node
        // contributes nothing to the top-down signal.
        let mut layer = make_layer(2, 2, Activation::Relu);
        layer.weights = vec![1.0, 0.0, 0.0, 1.0];

        layer.preactivations = vec![-1.0, 2.0]; // first is negative
        layer.errors = vec![10.0, 5.0];

        let td = layer.top_down_error();
        // modulated = [0 * 10, 1 * 5] = [0, 5]
        // W^T * [0, 5] with identity = [0, 5]
        assert!((td[0] - 0.0).abs() < 1e-12);
        assert!((td[1] - 5.0).abs() < 1e-12);
    }

    #[test]
    fn top_down_error_non_square_weights() {
        // dim=2, input_dim=3, so W is 2x3
        let mut layer = make_layer(2, 3, Activation::Relu);
        layer.weights = vec![
            1.0, 2.0, 3.0, //
            4.0, 5.0, 6.0, //
        ];
        layer.preactivations = vec![1.0, 1.0]; // both positive, f'=1
        layer.errors = vec![1.0, 1.0];

        // modulated = [1*1, 1*1] = [1, 1]
        // W^T * [1,1] = [1+4, 2+5, 3+6] = [5, 7, 9]
        let td = layer.top_down_error();
        assert!((td[0] - 5.0).abs() < 1e-12);
        assert!((td[1] - 7.0).abs() < 1e-12);
        assert!((td[2] - 9.0).abs() < 1e-12);
    }

    // ---------------------------------------------------------------
    // update_weights
    // ---------------------------------------------------------------

    #[test]
    fn weight_update_changes_weights() {
        let mut layer = make_layer(2, 2, Activation::Relu);
        layer.weights = vec![1.0, 0.0, 0.0, 1.0];
        layer.preactivations = vec![1.0, 1.0]; // positive => f'=1
        layer.errors = vec![1.0, 2.0];

        let weights_before = layer.weights.clone();
        let bias_before = layer.bias.clone();

        let x_above = vec![3.0, 4.0];
        layer.update_weights(&x_above, 0.1);

        // Weights should have changed
        assert_ne!(layer.weights, weights_before);
        // Bias should have changed
        assert_ne!(layer.bias, bias_before);
    }

    #[test]
    fn weight_update_correct_values() {
        let mut layer = make_layer(2, 2, Activation::Relu);
        layer.weights = vec![0.0, 0.0, 0.0, 0.0];
        layer.bias = vec![0.0, 0.0];
        layer.preactivations = vec![1.0, 2.0]; // positive => f'=1
        layer.errors = vec![1.0, 0.5];

        let lr = 0.01;
        let x_above = vec![2.0, 3.0];
        layer.update_weights(&x_above, lr);

        // grad[0] = f'(1) * 1 = 1; scaled = 0.01 * 1 = 0.01
        // W[0,0] += 0.01 * 2 = 0.02; W[0,1] += 0.01 * 3 = 0.03
        // b[0] += 0.01
        assert!((layer.weights[0] - 0.02).abs() < 1e-12);
        assert!((layer.weights[1] - 0.03).abs() < 1e-12);
        assert!((layer.bias[0] - 0.01).abs() < 1e-12);

        // grad[1] = f'(2) * 0.5 = 0.5; scaled = 0.01 * 0.5 = 0.005
        // W[1,0] += 0.005 * 2 = 0.01; W[1,1] += 0.005 * 3 = 0.015
        // b[1] += 0.005
        assert!((layer.weights[2] - 0.01).abs() < 1e-12);
        assert!((layer.weights[3] - 0.015).abs() < 1e-12);
        assert!((layer.bias[1] - 0.005).abs() < 1e-12);
    }

    #[test]
    fn weight_update_zero_error_no_change() {
        let mut layer = make_layer(2, 2, Activation::Relu);
        layer.weights = vec![1.0, 2.0, 3.0, 4.0];
        layer.bias = vec![0.5, 0.5];
        layer.preactivations = vec![1.0, 1.0];
        layer.errors = vec![0.0, 0.0]; // zero error => no update

        let weights_before = layer.weights.clone();
        let bias_before = layer.bias.clone();

        layer.update_weights(&[1.0, 1.0], 0.1);

        assert_eq!(layer.weights, weights_before);
        assert_eq!(layer.bias, bias_before);
    }

    #[test]
    fn weight_update_zero_lr_no_change() {
        let mut layer = make_layer(2, 2, Activation::Relu);
        layer.weights = vec![1.0, 2.0, 3.0, 4.0];
        layer.bias = vec![0.5, 0.5];
        layer.preactivations = vec![1.0, 1.0];
        layer.errors = vec![1.0, 1.0];

        let weights_before = layer.weights.clone();
        let bias_before = layer.bias.clone();

        layer.update_weights(&[1.0, 1.0], 0.0);

        assert_eq!(layer.weights, weights_before);
        assert_eq!(layer.bias, bias_before);
    }

    // ---------------------------------------------------------------
    // randomize_values
    // ---------------------------------------------------------------

    #[test]
    fn randomize_values_produces_nonzero_values() {
        let mut layer = make_layer(10, 5, Activation::Relu);
        assert!(layer.values.iter().all(|&v| v == 0.0));

        layer.randomize_values(123);
        // Very unlikely that all 10 random values are exactly 0
        assert!(layer.values.iter().any(|&v| v != 0.0));
    }

    #[test]
    fn randomize_values_is_deterministic() {
        let mut layer1 = make_layer(8, 4, Activation::Relu);
        let mut layer2 = make_layer(8, 4, Activation::Relu);

        layer1.randomize_values(42);
        layer2.randomize_values(42);

        assert_eq!(layer1.values, layer2.values);
    }

    #[test]
    fn randomize_values_bounded() {
        let mut layer = make_layer(100, 50, Activation::Tanh);
        layer.randomize_values(99);
        for &v in &layer.values {
            assert!(
                v.abs() <= 0.1,
                "Randomized value {} exceeds [-0.1, 0.1]",
                v
            );
        }
    }

    // ---------------------------------------------------------------
    // resize_input
    // ---------------------------------------------------------------

    #[test]
    fn resize_input_preserves_existing_weights() {
        let mut layer = make_layer(2, 3, Activation::Relu);
        let original_weights = layer.weights.clone();

        layer.resize_input(5);

        assert_eq!(layer.input_dim, 5);
        assert_eq!(layer.weights.len(), 2 * 5);

        // First 3 columns of each row should be unchanged
        for i in 0..2 {
            for j in 0..3 {
                assert_eq!(
                    layer.weights[i * 5 + j],
                    original_weights[i * 3 + j],
                    "Weight at ({}, {}) changed after resize",
                    i,
                    j
                );
            }
        }
    }

    #[test]
    fn resize_input_new_columns_are_nonzero() {
        let mut layer = make_layer(4, 2, Activation::Relu);
        layer.resize_input(6);

        // New columns (indices 2..6) should have at least some nonzero values
        let mut any_nonzero = false;
        for i in 0..4 {
            for j in 2..6 {
                if layer.weights[i * 6 + j] != 0.0 {
                    any_nonzero = true;
                }
            }
        }
        assert!(any_nonzero, "New weight columns should not all be zero");
    }

    #[test]
    fn resize_input_same_dim_is_noop() {
        let mut layer = make_layer(3, 4, Activation::Relu);
        let weights_before = layer.weights.clone();
        layer.resize_input(4);
        assert_eq!(layer.weights, weights_before);
        assert_eq!(layer.input_dim, 4);
    }

    #[test]
    #[should_panic(expected = "Cannot shrink input dimension")]
    fn resize_input_shrink_panics() {
        let mut layer = make_layer(3, 4, Activation::Relu);
        layer.resize_input(2);
    }

    // ---------------------------------------------------------------
    // Integration: predict -> compute_errors -> top_down -> update
    // ---------------------------------------------------------------

    #[test]
    fn full_cycle_predict_error_update() {
        let mut layer = make_layer(3, 2, Activation::Tanh);
        // Set known weights
        layer.weights = vec![
            0.5, -0.3, //
            0.1, 0.8, //
            -0.2, 0.4, //
        ];
        layer.bias = vec![0.0; 3];
        layer.values = vec![0.5, 0.5, 0.5];

        let x_above = vec![1.0, 1.0];

        // Predict
        let pred = layer.predict(&x_above);
        assert_eq!(pred.len(), 3);

        // Compute errors
        let errors = layer.compute_errors(&x_above).to_vec();
        assert_eq!(errors.len(), 3);

        // Top-down error
        let td = layer.top_down_error();
        assert_eq!(td.len(), 2);

        // Update weights
        let w_before = layer.weights.clone();
        layer.update_weights(&x_above, 0.01);

        // At least some weights should change (errors are nonzero)
        assert_ne!(layer.weights, w_before);
    }
}
