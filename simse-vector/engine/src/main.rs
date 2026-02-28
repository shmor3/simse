use simse_vector_engine::server::VectorServer;
use simse_vector_engine::transport::NdjsonTransport;

fn main() {
	tracing_subscriber::fmt()
		.with_writer(std::io::stderr)
		.with_env_filter(
			tracing_subscriber::EnvFilter::try_from_default_env()
				.unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
		)
		.init();

	let transport = NdjsonTransport::new();
	let mut server = VectorServer::new(transport);

	tracing::info!("simse-vector-engine ready");

	if let Err(e) = server.run() {
		tracing::error!("Server error: {}", e);
		std::process::exit(1);
	}
}
