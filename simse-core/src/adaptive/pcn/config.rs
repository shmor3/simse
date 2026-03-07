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
            Self::Relu => {
                if x > 0.0 {
                    1.0
                } else {
                    0.0
                }
            }
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
    pub layers: Vec<LayerConfig>,
    pub inference_steps: usize,
    pub learning_rate: f64,
    pub inference_rate: f64,
    pub batch_size: usize,
    pub max_batch_delay_ms: u64,
    pub channel_capacity: usize,
    pub auto_save_epochs: usize,
    pub max_topics: usize,
    pub max_tags: usize,
    pub temporal_amortization: bool,
    pub storage_path: Option<String>,
}

impl Default for PcnConfig {
    fn default() -> Self {
        Self {
            layers: vec![
                LayerConfig {
                    dim: 512,
                    activation: Activation::Relu,
                },
                LayerConfig {
                    dim: 256,
                    activation: Activation::Relu,
                },
                LayerConfig {
                    dim: 64,
                    activation: Activation::Tanh,
                },
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
                LayerConfig {
                    dim: 512,
                    activation: Activation::Relu,
                },
                LayerConfig {
                    dim: 128,
                    activation: Activation::Tanh,
                },
                LayerConfig {
                    dim: 32,
                    activation: Activation::Sigmoid,
                },
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
        assert!((Activation::Sigmoid.derivative(0.0) - 0.25).abs() < 1e-10);
        assert!((Activation::Tanh.derivative(0.0) - 1.0).abs() < 1e-10);
    }

    #[test]
    fn config_serialization_round_trip() {
        let config = PcnConfig::default();
        let json = serde_json::to_string(&config).unwrap();
        let restored: PcnConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.layers.len(), config.layers.len());
        assert_eq!(restored.inference_steps, config.inference_steps);
    }
}
