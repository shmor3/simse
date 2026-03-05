//! Temporal amortization: warm-start inference vs fresh randomization.
//!
//! Demonstrates that reusing latent states from the previous inference
//! (temporal amortization) reaches lower energy in fewer steps than
//! re-randomizing latents each time. This is especially effective for
//! temporally correlated inputs (sequential conversation turns).
//!
//! Run: `cargo run --example temporal_amortization`

use std::time::Instant;

use simse_pcn_engine::config::{Activation, LayerConfig, PcnConfig};
use simse_pcn_engine::network::PredictiveCodingNetwork;

fn main() {
    println!("=== Predictive Coding Network: Temporal Amortization ===\n");

    // ---------------------------------------------------------------
    // 1. Production-scale model
    // ---------------------------------------------------------------
    let input_dim = 2278; // 768 embedding + 500 topics + 1000 tags + 10 fixed
    let config = PcnConfig {
        layers: vec![
            LayerConfig { dim: 512, activation: Activation::Relu },
            LayerConfig { dim: 256, activation: Activation::Relu },
            LayerConfig { dim: 64, activation: Activation::Tanh },
        ],
        inference_steps: 20,
        learning_rate: 0.001,
        inference_rate: 0.1,
        temporal_amortization: false,
        ..Default::default()
    };

    println!("Model: [512, 256, 64] layers, {} input dims", input_dim);
    println!("Training on 20 correlated samples...\n");

    // ---------------------------------------------------------------
    // 2. Generate temporally correlated inputs
    //    Each input drifts slightly from the previous one (simulating
    //    a conversation that evolves topic over time).
    // ---------------------------------------------------------------
    let num_samples = 20;
    let mut inputs: Vec<Vec<f64>> = Vec::with_capacity(num_samples);
    let mut base = vec![0.0; input_dim];

    // Initialize with a pattern.
    for i in 0..input_dim {
        base[i] = ((i as f64) * 0.01).sin() * 0.5;
    }

    for s in 0..num_samples {
        let mut input = base.clone();
        // Small drift: each sample shifts slightly.
        for i in 0..input_dim {
            input[i] += ((s as f64) * 0.05 + (i as f64) * 0.001).cos() * 0.1;
        }
        inputs.push(input);
    }

    // ---------------------------------------------------------------
    // 3. Train the network (same for both comparisons)
    // ---------------------------------------------------------------
    let mut net_fresh = PredictiveCodingNetwork::new(input_dim, &config);
    for sample in &inputs {
        net_fresh.train_single_with_steps(sample, 20, false);
    }

    // Clone weights for amortized network.
    let mut net_amortized = net_fresh.clone();

    // ---------------------------------------------------------------
    // 4. Compare: fresh vs amortized at different step counts
    // ---------------------------------------------------------------
    println!("{:<8} {:<18} {:<18} {:<12}", "Steps", "Energy (Fresh)", "Energy (Amortized)", "Improvement");
    println!("{:-<8} {:-<18} {:-<18} {:-<12}", "", "", "", "");

    for steps in [5, 10, 15, 20, 30, 50] {
        // Fresh inference: randomize latents each time.
        let start = Instant::now();
        let mut total_fresh = 0.0;
        for sample in &inputs {
            total_fresh += net_fresh.infer(sample, steps);
        }
        let avg_fresh = total_fresh / num_samples as f64;
        let fresh_time = start.elapsed();

        // Amortized inference: preserve latents between samples.
        let start = Instant::now();
        let mut total_amortized = 0.0;
        for (i, sample) in inputs.iter().enumerate() {
            if i == 0 {
                // First sample: must randomize (no prior state).
                total_amortized += net_amortized.infer(sample, steps);
            } else {
                total_amortized += net_amortized.infer_amortized(sample, steps);
            }
        }
        let avg_amortized = total_amortized / num_samples as f64;
        let amortized_time = start.elapsed();

        let improvement = if avg_fresh > 0.0 {
            ((avg_fresh - avg_amortized) / avg_fresh) * 100.0
        } else {
            0.0
        };

        println!(
            "{:<8} {:<18.4} {:<18.4} {:<12}",
            steps,
            avg_fresh,
            avg_amortized,
            format!("{:+.1}%", improvement),
        );

        // Also show timing for the last row.
        if steps == 50 {
            println!();
            println!("Timing at {} steps ({} samples):", steps, num_samples);
            println!("  Fresh:     {:?}", fresh_time);
            println!("  Amortized: {:?}", amortized_time);
        }
    }

    println!("\nKey insight: Amortized inference reaches lower energy because");
    println!("latent states from the previous (similar) input provide a");
    println!("better starting point than random initialization.\n");
    println!("Done.");
}
