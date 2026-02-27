use anyhow::Result;
use clap::Parser;
use simse_engine::config::CliArgs;
use simse_engine::models::{ModelConfig, ModelRegistry, SamplingParams};
use simse_engine::server::{AcpServer, ServerConfig};
use simse_engine::transport::NdjsonTransport;

fn main() -> Result<()> {
    let args = CliArgs::parse();

    // Initialize logging to stderr (ACP protocol uses stdout exclusively)
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new(&args.log_level)),
        )
        .init();

    // Select compute device
    let device = match args.device.as_str() {
        #[cfg(feature = "cuda")]
        "cuda" => candle_core::Device::new_cuda(args.device_id)?,
        #[cfg(feature = "metal")]
        "metal" => candle_core::Device::new_metal(0)?,
        "cpu" => candle_core::Device::Cpu,
        other => {
            tracing::warn!("Unknown device '{}', falling back to CPU", other);
            candle_core::Device::Cpu
        }
    };

    tracing::info!(device = ?device, "Compute device selected");

    // Initialize model registry
    let mut registry = ModelRegistry::new(device);

    // Load text generation model
    tracing::info!(model = %args.model, "Loading text generation model");
    registry.load_generator(
        &args.model,
        &ModelConfig {
            filename: args.model_file.clone(),
            ..Default::default()
        },
    )?;

    // Load embedding model
    tracing::info!(model = %args.embedding_model, "Loading embedding model");
    registry.load_embedder(
        &args.embedding_model,
        &ModelConfig::default(),
    )?;

    // Create server config
    let config = ServerConfig {
        server_name: args.server_name,
        server_version: args.server_version,
        default_model: args.model,
        embedding_model: args.embedding_model,
        streaming: !args.no_streaming,
        default_sampling: SamplingParams {
            temperature: args.temperature,
            top_p: args.top_p,
            top_k: None,
            max_tokens: args.max_tokens,
            repeat_penalty: args.repeat_penalty,
            repeat_last_n: 64,
            stop_sequences: vec![],
        },
    };

    // Create transport and run server
    let transport = NdjsonTransport::new();
    let mut server = AcpServer::new(config, registry, transport);

    tracing::info!("simse-engine ACP server ready");
    server.run()
}
