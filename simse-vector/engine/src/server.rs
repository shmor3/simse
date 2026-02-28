use std::io::{self, BufRead};

use crate::protocol::*;
use crate::transport::NdjsonTransport;

pub struct VectorServer {
	transport: NdjsonTransport,
}

impl VectorServer {
	pub fn new(transport: NdjsonTransport) -> Self {
		Self { transport }
	}

	pub fn run(&mut self) -> Result<(), crate::error::VectorError> {
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

			self.transport.write_error(
				request.id,
				METHOD_NOT_FOUND,
				format!("Method not found: {}", request.method),
				None,
			);
		}

		Ok(())
	}
}
