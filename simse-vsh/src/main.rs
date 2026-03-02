use simse_vsh_engine::server::VshServer;
use simse_vsh_engine::transport::NdjsonTransport;

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
	let mut server = VshServer::new(transport);

	tracing::info!("simse-vsh-engine ready");

	if let Err(e) = server.run().await {
		tracing::error!("Server error: {}", e);
		std::process::exit(1);
	}
}
