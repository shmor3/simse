use clap::Parser;

#[derive(Parser, Debug)]
#[command(name = "simse-engine", about = "ACP-compatible ML inference server using Candle")]
pub struct CliArgs {
    /// Text generation model (HuggingFace repo ID or local path)
    #[arg(long, default_value = "bartowski/Llama-3.2-3B-Instruct-GGUF", env = "SIMSE_ENGINE_MODEL")]
    pub model: String,

    /// Specific model filename (e.g., Llama-3.2-3B-Instruct-Q4_K_M.gguf)
    #[arg(long)]
    pub model_file: Option<String>,

    /// Tokenizer source (HuggingFace repo ID or local path to tokenizer.json).
    /// Auto-detected from the model repo if not specified.
    #[arg(long, env = "SIMSE_ENGINE_TOKENIZER")]
    pub tokenizer: Option<String>,

    /// Embedding model (HuggingFace repo ID or local path)
    #[arg(long, default_value = "nomic-ai/nomic-embed-text-v1.5", env = "SIMSE_ENGINE_EMBEDDING_MODEL")]
    pub embedding_model: String,

    /// TEI server URL for remote embeddings (e.g., http://localhost:8080)
    #[arg(long, env = "SIMSE_ENGINE_TEI_URL")]
    pub tei_url: Option<String>,

    /// Device: "cpu", "cuda", "metal"
    #[arg(long, default_value = "cpu", env = "SIMSE_ENGINE_DEVICE")]
    pub device: String,

    /// CUDA device ordinal (when --device cuda)
    #[arg(long, default_value = "0")]
    pub device_id: usize,

    /// Server name in ACP initialize response
    #[arg(long, default_value = "simse-engine")]
    pub server_name: String,

    /// Server version
    #[arg(long, default_value = env!("CARGO_PKG_VERSION"))]
    pub server_version: String,

    /// Temperature for generation
    #[arg(long, default_value = "0.7")]
    pub temperature: f64,

    /// Top-p (nucleus sampling)
    #[arg(long)]
    pub top_p: Option<f64>,

    /// Maximum tokens to generate
    #[arg(long, default_value = "2048")]
    pub max_tokens: usize,

    /// Repeat penalty
    #[arg(long, default_value = "1.1")]
    pub repeat_penalty: f32,

    /// Disable streaming (send full response only)
    #[arg(long)]
    pub no_streaming: bool,

    /// Generation timeout in seconds (wall-clock limit per request)
    #[arg(long, default_value = "300", env = "SIMSE_ENGINE_GENERATION_TIMEOUT")]
    pub generation_timeout: u64,

    /// Log level (trace, debug, info, warn, error)
    #[arg(long, default_value = "info", env = "SIMSE_ENGINE_LOG_LEVEL")]
    pub log_level: String,
}
